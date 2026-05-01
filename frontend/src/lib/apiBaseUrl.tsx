import { createContext, useContext } from 'react';

const ApiBaseUrlContext = createContext<string | undefined>(undefined);

export function useApiBaseUrl(): string {
  const apiBaseUrl = useContext(ApiBaseUrlContext);
  if (apiBaseUrl === undefined) {
    throw new Error('useApiBaseUrl must be used within ApiBaseUrlProvider');
  }
  return apiBaseUrl;
}

export const ApiBaseUrlProvider = ApiBaseUrlContext.Provider;
