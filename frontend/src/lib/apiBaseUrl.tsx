import { createContext, useContext } from 'react';

const ApiBaseUrlContext = createContext<string>('');

export function useApiBaseUrl(): string {
  return useContext(ApiBaseUrlContext);
}

export const ApiBaseUrlProvider = ApiBaseUrlContext.Provider;
