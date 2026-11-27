import { handleBffRequest } from "@/lib/server/bff";

export const dynamic = "force-dynamic";

type RouteContext = { params: Promise<{ service: string; path?: string[] }> };

async function handle(request: Request, context: RouteContext): Promise<Response> {
  const { service, path = [] } = await context.params;
  return handleBffRequest(request, service, path);
}

export const GET = handle;
export const HEAD = handle;
export const POST = handle;
export const PUT = handle;
export const PATCH = handle;
export const DELETE = handle;
export const OPTIONS = handle;
