import { generateTraceparent } from './logger';

export class HttpError extends Error {
  public readonly body: unknown;

  constructor(
    public readonly status: number,
    message: string,
    body?: unknown,
  ) {
    super(message);
    this.name = 'HttpError';
    this.body = body;
  }
}

type ErrorResponseBody = {
  message?: string;
};

export class AuthenticationError extends Error {
  constructor() {
    super('401: Unauthorized');
    this.name = 'AuthenticationError';
  }
}

export function buildUrl(apiBaseUrl: string, path: string): string {
  if (!apiBaseUrl) return path;
  const base = apiBaseUrl.endsWith('/') ? apiBaseUrl.slice(0, -1) : apiBaseUrl;
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${base}${normalizedPath}`;
}

type RequestOptions = {
  allowedErrorStatuses?: number[];
  timeoutMs?: number;
};

async function request<TRes>(
  method: string,
  apiBaseUrl: string,
  path: string,
  body?: unknown,
  options?: RequestOptions,
): Promise<TRes> {
  const headers: Record<string, string> = {
    Accept: 'application/json',
    traceparent: generateTraceparent(),
  };

  if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
  }

  const response = await fetch(buildUrl(apiBaseUrl, path), {
    method,
    headers,
    credentials: 'include',
    body: body !== undefined ? JSON.stringify(body) : undefined,
    signal: options?.timeoutMs ? AbortSignal.timeout(options.timeoutMs) : undefined,
  });

  if (response.status === 401) {
    throw new AuthenticationError();
  }

  const responseBody = await parseResponseBody(response);

  if (!response.ok && !options?.allowedErrorStatuses?.includes(response.status)) {
    throw new HttpError(
      response.status,
      getErrorMessage(method, path, response.status, responseBody),
      responseBody,
    );
  }

  if (response.status === 204) {
    return undefined as TRes;
  }

  if (responseBody === undefined) {
    throw new HttpError(response.status, `${method} ${path}: invalid JSON response`);
  }

  return responseBody as TRes;
}

async function parseResponseBody(response: Response): Promise<unknown> {
  if (response.status === 204) {
    return undefined;
  }

  try {
    return await response.json();
  } catch {
    return undefined;
  }
}

function getErrorMessage(method: string, path: string, status: number, body: unknown): string {
  if (isErrorResponseBody(body) && typeof body.message === 'string' && body.message.trim()) {
    return body.message;
  }

  return `${method} ${path} failed: ${status}`;
}

function isErrorResponseBody(body: unknown): body is ErrorResponseBody {
  return typeof body === 'object' && body !== null && 'message' in body;
}

export function get<TRes>(apiBaseUrl: string, path: string, options?: RequestOptions): Promise<TRes> {
  return request<TRes>('GET', apiBaseUrl, path, undefined, options);
}

export function post<TReq, TRes>(
  apiBaseUrl: string,
  path: string,
  body?: TReq,
  options?: RequestOptions,
): Promise<TRes> {
  return request<TRes>('POST', apiBaseUrl, path, body, options);
}

export function patch<TReq, TRes>(
  apiBaseUrl: string,
  path: string,
  body: TReq,
  options?: RequestOptions,
): Promise<TRes> {
  return request<TRes>('PATCH', apiBaseUrl, path, body, options);
}

export function put<TReq, TRes>(
  apiBaseUrl: string,
  path: string,
  body: TReq,
  options?: RequestOptions,
): Promise<TRes> {
  return request<TRes>('PUT', apiBaseUrl, path, body, options);
}

export function del<TRes>(apiBaseUrl: string, path: string, options?: RequestOptions): Promise<TRes> {
  return request<TRes>('DELETE', apiBaseUrl, path, undefined, options);
}
