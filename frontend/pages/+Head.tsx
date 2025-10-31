import fontsCss from "@/shared/styles/fonts.css?inline";
import oswaldWoff2 from "@fontsource-variable/oswald/files/oswald-latin-wght-normal.woff2?url";

export default function HeadDefault() {
    return (
        <>
            <meta charSet="UTF-8" />
            <meta name="viewport" content="width=device-width, initial-scale=1.0" />
            <link rel="preload" href={oswaldWoff2} as="font" type="font/woff2" crossOrigin="anonymous" />
            {/* Inlined font definitions */}
            <style dangerouslySetInnerHTML={{ __html: fontsCss }} />
            {/* Global styles for initial render */}
            <style>{`
        body::before {
          content: '';
          position: absolute;
          top: 0;
          left: 0;
          width: 100%;
          height: 100%;
          filter: blur(4px) brightness(0.85);
          transform: scale(1.05);
          z-index: -2;
        }

        body::after {
          content: '';
          position: absolute;
          top: 0;
          left: 0;
          width: 100%;
          height: 100%;
          background: rgba(255, 255, 255, 0.3);
          backdrop-filter: blur(1px);
          z-index: -1;
        }
      `}</style>
        </>
    );
}
