import { test } from 'node:test';
import assert from 'node:assert/strict';
import { PeakHold } from '../web/peak-hold.js';

const reading = (pitchText, strength) => ({ pitchText, strength });

test('最も強い直近の読みを保持する', () => {
  const hold = new PeakHold(10_000);
  hold.push(reading('C  4', 0.2), 0);
  hold.push(reading('G  4', 0.9), 1);
  hold.push(reading('D  4', 0.1), 2);

  assert.equal(hold.peak().pitchText, 'G  4');
});

test('保持時間を過ぎた読みは忘れる', () => {
  const hold = new PeakHold(50);
  hold.push(reading('A  4', 1.0), 0);
  hold.evictExpired(80);

  assert.equal(hold.peak(), null);
});
