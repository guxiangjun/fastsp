import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

interface AudioVisualizerProps {
    isRecording: boolean;
    className?: string;
}

class Particle {
    x: number;
    y: number;
    radius: number;
    color: string;
    velocity: { x: number; y: number };
    alpha: number;
    baseRadius: number;

    constructor(canvasWidth: number, canvasHeight: number) {
        this.x = canvasWidth / 2;
        this.y = canvasHeight / 2;
        this.baseRadius = Math.random() * 20 + 10;
        this.radius = this.baseRadius;
        this.color = `hsl(${220 + Math.random() * 40}, 70%, 50%)`; // Indigo/Blue range
        this.velocity = {
            x: (Math.random() - 0.5) * 2,
            y: (Math.random() - 0.5) * 2
        };
        this.alpha = Math.random() * 0.5 + 0.1;
    }

    update(level: number, width: number, height: number) {
        // Excited state based on audio level
        const speedMultiplier = 1 + level * 5;
        this.x += this.velocity.x * speedMultiplier;
        this.y += this.velocity.y * speedMultiplier;

        // Contain within circle
        const dist = Math.sqrt((this.x - width / 2) ** 2 + (this.y - height / 2) ** 2);
        if (dist > 60 + level * 40) {
            // Soft tether back to center
            const angle = Math.atan2(this.y - height / 2, this.x - width / 2);
            this.velocity.x -= Math.cos(angle) * 0.1;
            this.velocity.y -= Math.sin(angle) * 0.1;
        }

        // Pulse radius
        this.radius = this.baseRadius * (1 + level * 2);

        // Fade alpha based on level (brighter when loud)
        // this.alpha = 0.3 + level * 0.7; // Too flicker-y?
    }

    draw(ctx: CanvasRenderingContext2D) {
        ctx.beginPath();
        ctx.arc(this.x, this.y, this.radius, 0, Math.PI * 2);
        ctx.fillStyle = this.color;
        ctx.globalAlpha = this.alpha;
        ctx.fill();
        ctx.globalAlpha = 1.0;
    }
}

export function AudioVisualizer({ isRecording, className }: AudioVisualizerProps) {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const audioLevelRef = useRef(0);
    const animationFrameRef = useRef<number | null>(null);
    const particlesRef = useRef<Particle[]>([]);

    useEffect(() => {
        // Initialize particles
        if (particlesRef.current.length === 0) {
            for (let i = 0; i < 20; i++) {
                particlesRef.current.push(new Particle(160, 160)); // Assumed size
            }
        }

        const unlisten = listen<number>("audio_level", (event) => {
            // Smooth audio level?
            // event.payload is RMS (0.0 to 1.0 usually small like 0.01 - 0.2)
            // boost it
            const target = Math.min(event.payload * 5, 1.0);
            audioLevelRef.current = audioLevelRef.current * 0.7 + target * 0.3;
        });

        return () => {
            unlisten.then(f => f());
        };
    }, []);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        const render = () => {
            if (!isRecording) {
                // If not recording, just clear or draw idle?
                // Let's keep drawing idle for smooth transition
                audioLevelRef.current = audioLevelRef.current * 0.9;
            }

            ctx.clearRect(0, 0, canvas.width, canvas.height);

            // Composite mode for "cloud" look
            ctx.globalCompositeOperation = "screen";

            // Center glow
            const centerX = canvas.width / 2;
            const centerY = canvas.height / 2;

            // Draw particles
            particlesRef.current.forEach(p => {
                p.update(audioLevelRef.current, canvas.width, canvas.height);
                p.draw(ctx);
            });

            // Draw core energy
            const coreRadius = 30 + audioLevelRef.current * 30;
            const gradient = ctx.createRadialGradient(centerX, centerY, 0, centerX, centerY, coreRadius);
            gradient.addColorStop(0, "rgba(255, 255, 255, 0.8)");
            gradient.addColorStop(0.5, "rgba(99, 102, 241, 0.4)"); // Indigo
            gradient.addColorStop(1, "rgba(99, 102, 241, 0)");

            ctx.fillStyle = gradient;
            ctx.beginPath();
            ctx.arc(centerX, centerY, coreRadius, 0, Math.PI * 2);
            ctx.fill();

            ctx.globalCompositeOperation = "source-over";

            animationFrameRef.current = requestAnimationFrame(render);
        };

        render();

        return () => {
            if (animationFrameRef.current) cancelAnimationFrame(animationFrameRef.current);
        };
    }, [isRecording]);

    return (
        <canvas
            ref={canvasRef}
            width={300}
            height={300}
            className={`pointer-events-none rounded-full ${className || "w-32 h-32"}`}
        />
    );
}
