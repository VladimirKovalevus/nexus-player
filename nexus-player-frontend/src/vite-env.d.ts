/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<Record<string, unknown>, Record<string, unknown>, unknown>;
  export default component;
}

declare module "butterchurn/dist/butterchurn.min.js" {
  export interface ButterchurnVisualizer {
    gl?: WebGLRenderingContext | WebGL2RenderingContext;
    loadPreset: (preset: unknown, blendTime: number) => void;
    loseGLContext?: () => void;
    setRendererSize: (width: number, height: number) => void;
    renderRaw?: (levels: unknown, deltaTime: number) => void;
    render: (opts?: unknown) => void;
    [key: string]: unknown;
  }
  const butterchurn: {
    createVisualizer: (
      ctx: AudioContext | null,
      canvas: HTMLCanvasElement,
      opts: Record<string, unknown>,
    ) => ButterchurnVisualizer;
  };
  export default butterchurn;
}

declare module "butterchurn-presets" {
  const presets: {
    getPresets: () => Record<string, unknown>;
  };
  export default presets;
}
