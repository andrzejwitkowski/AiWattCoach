export class ApiError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ApiError';
  }
}

export type JsonResponse<T> = {
  ok: boolean;
  status: number;
  body: T;
};

type GetJsonResponseOptions = {
  credentials?: RequestCredentials;
};

export async function getJsonResponse<T>(
  url: string,
  options: GetJsonResponseOptions = {}
): Promise<JsonResponse<T>> {
  const requestInit: RequestInit = {
    method: 'GET',
    headers: {
      Accept: 'application/json'
    }
  };

  if (options.credentials) {
    requestInit.credentials = options.credentials;
  }

  const response = await fetch(url, requestInit);

  let body: T;

  try {
    body = (await response.json()) as T;
  } catch {
    throw new ApiError(`Response from ${url} is not valid JSON`);
  }

  return {
    ok: response.ok,
    status: response.status,
    body
  };
}
