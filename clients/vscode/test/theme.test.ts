import * as assert from 'node:assert/strict';
import test from 'node:test';

import { ColorThemeKind, themeKind } from '../src/theme.js';

test('each editor theme kind is named for the server', () => {
  assert.equal(themeKind(ColorThemeKind.Light), 'light');
  assert.equal(themeKind(ColorThemeKind.Dark), 'dark');
});

test('both high contrast kinds collapse into one', () => {
  // The server sends no colour under high contrast, so the light and dark
  // variants have nothing left to tell apart.
  assert.equal(themeKind(ColorThemeKind.HighContrast), 'highContrast');
  assert.equal(themeKind(ColorThemeKind.HighContrastLight), 'highContrast');
});

test('a kind this extension has never heard of reads as dark', () => {
  // A kind the editor adds later must not crash the payload, and dark is
  // what the server assumes for anything it cannot place.
  assert.equal(themeKind(99), 'dark');
  assert.equal(themeKind(0), 'dark');
  assert.equal(themeKind(Number.NaN), 'dark');
});
