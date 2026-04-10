/**
 * API response envelope returned by generated clients.
 *
 * @template TData
 * @property {TData} data - Parsed response payload.
 * @property {number} status - HTTP status code.
 */
export interface ApiResponse<TData> {
  data: TData
  status: number
}

/**
 * Request a JSON resource and decode the response body.
 *
 * @param {URL | string} input - Request URL.
 * @param {RequestInit} [init] - Optional fetch options.
 * @returns {Promise<ApiResponse<TData>>} Parsed API response.
 * @throws {TypeError} When the response body cannot be decoded.
 */
export async function requestJson<TData>(
  input: URL | string,
  init?: RequestInit
): Promise<ApiResponse<TData>> {
  const response = await fetch(input, init)
  const data = await response.json() as TData
  return { data, status: response.status }
}

/**
 * Build a typed endpoint helper.
 *
 * @param baseURL - Base URL for the API service.
 * @returns A function that resolves endpoint paths against the base URL.
 */
export function createEndpoint(baseURL: URL) {
  return (path: string) => new URL(path, baseURL)
}
