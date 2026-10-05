/**
 * artGradient.spec.ts — regresión del gradiente de reserva compartido (R14).
 *
 * La misma pista debe conservar su color en Library, Playlists, Descargas,
 * Búsqueda, las fichas y Ahora suena: por eso el gradiente sale del id y no de
 * un aleatorio, y de una única función compartida.
 */
import { describe, it, expect } from 'vitest';
import { getArtGradient } from '@/utils/artGradient';

describe('getArtGradient', () => {
    it('es estable para el mismo id', () => {
        expect(getArtGradient(1)).toBe(getArtGradient(1));
        expect(getArtGradient(12345)).toBe(getArtGradient(12345));
    });

    it('acepta un id numérico escrito como texto (lo que llega en algunas filas)', () => {
        expect(getArtGradient('42')).toBe(getArtGradient(42));
    });

    it('nunca devuelve undefined para datos ausentes', () => {
        expect(getArtGradient(null)).toMatch(/^bg-gradient-to-br/);
        expect(getArtGradient(undefined)).toMatch(/^bg-gradient-to-br/);
        expect(getArtGradient('')).toMatch(/^bg-gradient-to-br/);
        expect(getArtGradient('abc')).toMatch(/^bg-gradient-to-br/);
    });

    it('cicla por un conjunto cerrado de gradientes', () => {
        const seen = new Set<string>();
        for (let id = 0; id < 64; id++) seen.add(getArtGradient(id));
        expect(seen.size).toBeGreaterThan(1);
        expect(seen.size).toBeLessThanOrEqual(8);
    });

    it('cicla con el id en lugar de desbordar el índice', () => {
        expect(getArtGradient(0)).toBe(getArtGradient(8));
        expect(getArtGradient(3)).toBe(getArtGradient(11));
    });
});