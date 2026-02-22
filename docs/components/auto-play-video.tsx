'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { Play } from 'lucide-react';

export function AutoPlayVideo({
  src,
  poster,
  className,
}: {
  src: string;
  poster?: string;
  className?: string;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [showPlay, setShowPlay] = useState(false);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    // Ensure attributes are set at DOM level (Safari can be picky with React attrs)
    video.setAttribute('playsinline', '');
    video.setAttribute('webkit-playsinline', '');
    video.muted = true;
    video.playsInline = true;

    const tryPlay = async () => {
      try {
        await video.play();
        setShowPlay(false);
      } catch {
        // Autoplay blocked — show play button
        setShowPlay(true);
      }
    };

    tryPlay();

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) tryPlay();
      },
      { threshold: 0.3 },
    );
    observer.observe(video);

    return () => observer.disconnect();
  }, []);

  const handleTap = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    video.muted = true;
    video.play().then(() => setShowPlay(false)).catch(() => {});
  }, []);

  return (
    <div className="relative" onClick={handleTap}>
      <video
        ref={videoRef}
        autoPlay
        loop
        muted
        playsInline
        preload="auto"
        poster={poster}
        className={className}
      >
        <source src={src} type="video/mp4" />
      </video>
      {showPlay && (
        <button
          aria-label="Play video"
          className="absolute inset-0 flex items-center justify-center bg-black/40 transition-opacity"
        >
          <div className="rounded-full bg-white/20 backdrop-blur-sm p-3">
            <Play className="w-8 h-8 text-white fill-white" />
          </div>
        </button>
      )}
    </div>
  );
}
