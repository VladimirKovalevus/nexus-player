import { i18n } from '@/i18n';

export interface CommandErrorLike {
  code: string;
  message: string;
  provider?: string;
  url?: string;
}

export function isCommandError(e: unknown): e is CommandErrorLike {
  return (
    typeof e === 'object' &&
    e !== null &&
    typeof (e as { code?: unknown }).code === 'string' &&
    typeof (e as { message?: unknown }).message === 'string'
  );
}

export function errorMessage(e: unknown): string {
  if (isCommandError(e)) return e.message;
  if (e instanceof Error) return e.message;
  if (typeof e === 'string') return e;
  return i18n.global.t('common.unknownError');
}
