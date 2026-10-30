import { expect, test } from 'vitest';
import { page } from '../../../lib/grade.ts';

test('the Post is dated the day it was given', () => {
	expect(page('/blog/hello/').querySelector('time[datetime]')?.getAttribute('datetime')).toBe('2026-01-05');
});
