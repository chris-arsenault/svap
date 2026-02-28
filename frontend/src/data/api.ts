/**
 * Shared API helpers with auth token handling.
 *
 * Both pipelineStore and ManagementView import from here
 * instead of maintaining separate fetch+auth logic.
 */

import { config } from "../config";
import { getToken } from "../auth";

export const API_BASE = config.apiBaseUrl || "/api";

export async function apiPost(path: string, body?: unknown): Promise<unknown> {
  const token = await getToken();
  const headers: Record<string, string> = {};
  if (body !== undefined) headers["Content-Type"] = "application/json";
  if (token) headers["Authorization"] = `Bearer ${token}`;
  const options: RequestInit = { method: "POST", headers };
  if (body !== undefined) options.body = JSON.stringify(body);
  const res = await fetch(`${API_BASE}${path}`, options);
  if (!res.ok) throw new Error(`${path} failed: ${res.status}`);
  const text = await res.text();
  return text ? JSON.parse(text) : {};
}

export async function apiGet(path: string, options?: { signal?: AbortSignal }): Promise<Response> {
  const token = await getToken();
  const headers: Record<string, string> = {};
  if (token) headers["Authorization"] = `Bearer ${token}`;
  return fetch(`${API_BASE}${path}`, { headers, ...options });
}
