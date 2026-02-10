import { getPageImage, source } from '@/lib/source';
import { notFound } from 'next/navigation';
import { ImageResponse } from 'next/og';

export const revalidate = false;

export async function GET(_req: Request, { params }: RouteContext<'/og/docs/[...slug]'>) {
  const { slug } = await params;
  const page = source.getPage(slug.slice(0, -1));
  if (!page) notFound();

  return new ImageResponse(
    (
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'center',
          width: '100%',
          height: '100%',
          padding: '60px 80px',
          background: 'linear-gradient(135deg, #0f0f17 0%, #1a1a2e 100%)',
          fontFamily: 'Inter, sans-serif',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 40 }}>
          <svg
            width="48"
            height="48"
            viewBox="0 0 1024 1024"
            xmlns="http://www.w3.org/2000/svg"
          >
            <rect width="1024" height="1024" rx="180" ry="180" fill="#0f0f17" />
            <path
              d="M256 768 Q512 256 768 768"
              stroke="#8B5CF6"
              strokeWidth="64"
              fill="none"
              strokeLinecap="round"
            />
          </svg>
          <span style={{ color: '#8B5CF6', fontSize: 28, marginLeft: 16, fontWeight: 600 }}>
            NomadFlow
          </span>
        </div>
        <div style={{ color: '#ffffff', fontSize: 48, fontWeight: 700, lineHeight: 1.2, marginBottom: 16 }}>
          {page.data.title}
        </div>
        {page.data.description && (
          <div style={{ color: '#8C8C99', fontSize: 24, lineHeight: 1.4 }}>
            {page.data.description}
          </div>
        )}
      </div>
    ),
    {
      width: 1200,
      height: 630,
    },
  );
}

export function generateStaticParams() {
  return source.getPages().map((page) => ({
    lang: page.locale,
    slug: getPageImage(page).segments,
  }));
}
