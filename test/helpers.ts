export const response = (body: BodyInit | null, init: ResponseInit = {}) => new Response(body, init);

export const protoResponse = (bytes: Uint8Array, init: ResponseInit = {}) =>
  response(bytes as BodyInit, { status: 200, statusText: "OK", ...init });

export const streamResponse = (chunks: readonly Uint8Array[], init: ResponseInit = {}) =>
  response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        chunks.forEach((chunk) => controller.enqueue(chunk));
        controller.close();
      }
    }),
    { status: 200, statusText: "OK", ...init }
  );

export const collect = async <T>(iterable: AsyncIterable<T>): Promise<readonly T[]> => {
  const values: T[] = [];
  for await (const value of iterable) values.push(value);
  return values;
};
