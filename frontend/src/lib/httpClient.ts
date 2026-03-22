export class HttpError extends Error {
  constructor(
    public readonly status: number,
    message: string
  ) {
    super(message);
    this.name = 'HttpError';
  }
}

export class AuthenticationError extends Error {
  constructor() {
    super('401: Unauthorized');
    this.name = 'AuthenticationError';
  }
}

function buildUrl(apiBaseUrl: string, path: string): string {
  if (!apiBaseUrl) return path;
  const base = apiBaseUrl.endsWith('/') ? apiBaseUrl.slice(0, -1) : apiBaseUrl;
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${base}${normalizedPath}`;
}

async function request<TRes>(
  method: string,
  apiBaseUrl: string,
  path: string,
  body?: unknown
): Promise<TRes> {
  const headers: Record<string, string> = {
    Accept: 'application/json',
  };

  if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
  }

  const response = await fetch(buildUrl(apiBaseUrl, path), {
    method,
    headers,
    credentials: 'include',
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });

  if (response.status === 401) {
    throw new AuthenticationError();
  }

  if (!response.ok) {
    throw new HttpError(response.status, `${method} ${path} failed: ${response.status}`);
  }

  if (response.status === 204) {
    return undefined as TRes;
  }

  try {
    return (await response.json()) as TRes;
  } catch {
    throw new HttpError(response.status, `${method} ${path}: invalid JSON response`);
  }
}

export function get<TRes>(apiBaseUrl: string, path: string): Promise<TRes> {
  return request<TRes>('GET', apiBaseUrl, path);
}

export function post<TReq, TRes>(
  apiBaseUrl: string,
  path: string,
  body: TReq
): Promise<TRes> {
  return request<TRes>('POST', apiBaseUrl, path, body);
}

export function patch<TReq, TRes>(
  apiBaseUrl: string,
  path: string,
  body: TReq
): Promise<TRes> {
  return request<TRes>('PATCH', apiBaseUrl, path, body);
}

export function put<TReq, TRes>(
  apiBaseUrl: string,
  path: string,
  body: TReq
): Promise<TRes> {
  return request<TRes>('PUT', apiBaseUrl, path, body);
}

export function del<TRes>(apiBaseUrl: string, path: string): Promise<TRes> {
  return request<TRes>('DELETE', apiBaseUrl, path);
}
