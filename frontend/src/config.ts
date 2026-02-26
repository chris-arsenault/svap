type RuntimeConfig = {
  apiBaseUrl?: string;
  cognitoUserPoolId?: string;
  cognitoClientId?: string;
};

declare global {
  interface Window {
    __APP_CONFIG__?: RuntimeConfig;
  }
}

const runtimeConfig = typeof window !== "undefined" ? window.__APP_CONFIG__ : undefined;
const readRuntime = (value?: string) => (value && value.trim().length > 0 ? value : undefined);

export const config = {
  apiBaseUrl: readRuntime(runtimeConfig?.apiBaseUrl) ?? import.meta.env.VITE_API_BASE_URL ?? "",
  cognitoUserPoolId: readRuntime(runtimeConfig?.cognitoUserPoolId) ?? import.meta.env.VITE_COGNITO_USER_POOL_ID ?? "",
  cognitoClientId: readRuntime(runtimeConfig?.cognitoClientId) ?? import.meta.env.VITE_COGNITO_CLIENT_ID ?? "",
};
