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

export async function getJsonResponse<T>(url: string): Promise<JsonResponse<T>> {
  const response = await fetch(url, {
    method: 'GET',
    headers: {
      Accept: 'application/json'
    }
  });

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
