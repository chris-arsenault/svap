"""
AWS Bedrock client wrapper for SVAP pipeline.

Handles prompt rendering, API calls, structured output parsing,
retries, and token counting.
"""

import json
import logging
import time
from pathlib import Path

logger = logging.getLogger(__name__)

PROMPTS_DIR = Path(__file__).parent / "prompts"


class BedrockClient:
    """Wrapper around AWS Bedrock for Claude API calls."""

    def __init__(self, config: dict):
        import boto3
        from botocore.config import Config

        self.config = config["bedrock"]
        self.client = boto3.client(
            "bedrock-runtime",
            region_name=self.config["region"],
            config=Config(read_timeout=300, retries={"max_attempts": 0}),
        )
        self.model_id = self.config["model_id"]
        self.max_tokens = self.config.get("max_tokens", 4096)
        self.default_temperature = self.config.get("temperature", 0.2)
        self.retry_attempts = self.config.get("retry_attempts", 3)
        self.retry_delay = self.config.get("retry_delay_seconds", 5)

    def invoke(
        self,
        prompt: str,
        system: str = "",
        temperature: float | None = None,
        max_tokens: int | None = None,
    ) -> str:
        """Send a prompt to Claude via Bedrock and return the text response."""
        temp = temperature if temperature is not None else self.default_temperature
        tokens = max_tokens or self.max_tokens

        messages = [{"role": "user", "content": [{"type": "text", "text": prompt}]}]

        body = {
            "anthropic_version": "bedrock-2023-05-31",
            "max_tokens": tokens,
            "temperature": temp,
            "messages": messages,
        }
        if system:
            body["system"] = [{"type": "text", "text": system}]

        for attempt in range(self.retry_attempts):
            try:
                response = self.client.invoke_model(
                    modelId=self.model_id,
                    contentType="application/json",
                    accept="application/json",
                    body=json.dumps(body),
                )
                result = json.loads(response["body"].read())
                # Extract text from content blocks
                text_parts = [
                    block["text"]
                    for block in result.get("content", [])
                    if block.get("type") == "text"
                ]
                return "\n".join(text_parts)

            except Exception as e:
                if attempt < self.retry_attempts - 1:
                    wait = self.retry_delay * (2**attempt)
                    logger.warning(
                        "Bedrock call failed (attempt %d): %s. Retrying in %ds...",
                        attempt + 1, e, wait,
                    )
                    time.sleep(wait)
                else:
                    raise RuntimeError(
                        f"Bedrock call failed after {self.retry_attempts} attempts: {e}"
                    ) from e

    def invoke_json(
        self,
        prompt: str,
        system: str = "",
        temperature: float | None = None,
        max_tokens: int | None = None,
    ) -> dict:
        """Invoke and parse JSON from the response. Handles markdown fences."""
        raw = self.invoke(prompt, system=system, temperature=temperature, max_tokens=max_tokens)
        return _parse_json_response(raw)

    def render_prompt(self, template_name: str, **variables) -> str:
        """Load a prompt template and fill in variables."""
        template_path = PROMPTS_DIR / template_name
        if not template_path.exists():
            raise FileNotFoundError(f"Prompt template not found: {template_path}")
        template = template_path.read_text()
        for key, value in variables.items():
            template = template.replace(f"{{{key}}}", str(value))
        return template


def _parse_json_response(text: str) -> dict:
    """Extract JSON from an LLM response, handling markdown fences and preamble."""
    # Strip markdown json fences
    cleaned = text.strip()
    if cleaned.startswith("```json"):
        cleaned = cleaned[7:]
    elif cleaned.startswith("```"):
        cleaned = cleaned[3:]
    if cleaned.endswith("```"):
        cleaned = cleaned[:-3]
    cleaned = cleaned.strip()

    # Try direct parse
    try:
        return json.loads(cleaned)
    except json.JSONDecodeError:
        pass

    # Try to find JSON object or array in the text
    for start_char, end_char in [("{", "}"), ("[", "]")]:
        start = cleaned.find(start_char)
        end = cleaned.rfind(end_char)
        if start != -1 and end != -1 and end > start:
            try:
                return json.loads(cleaned[start : end + 1])
            except json.JSONDecodeError:
                continue

    raise ValueError(f"Could not parse JSON from response:\n{text[:500]}")
