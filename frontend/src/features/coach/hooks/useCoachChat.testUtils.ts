import { vi } from 'vitest';

import type { WorkoutSummary } from '../types';

export class FakeWebSocket {
  static instances: FakeWebSocket[] = [];
  static failNextConnection = false;
  static OPEN = 1;
  static CLOSED = 3;

  public readyState = 1;
  private listeners = new Map<string, Set<(event?: MessageEvent) => void>>();

  constructor(public readonly url: string) {
    FakeWebSocket.instances.push(this);
    queueMicrotask(() => {
      if (FakeWebSocket.failNextConnection) {
        FakeWebSocket.failNextConnection = false;
        this.emit('error');
        this.close();
        return;
      }

      this.emit('open');
    });
  }

  addEventListener(type: string, listener: (event?: MessageEvent) => void) {
    const current = this.listeners.get(type) ?? new Set();
    current.add(listener);
    this.listeners.set(type, current);
  }

  close() {
    this.readyState = 3;
    this.emit('close');
  }

  send = vi.fn();

  emit(type: string, event?: MessageEvent) {
    this.listeners.get(type)?.forEach((listener) => {
      listener(event);
    });
  }
}

export const summaryFixture: WorkoutSummary = {
  id: 'summary-1',
  workoutId: '101',
  rpe: 7,
  messages: [],
  createdAtEpochSeconds: 1,
  updatedAtEpochSeconds: 2,
  savedAtEpochSeconds: null,
};

export function installFakeWebSocket() {
  global.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
}

export function resetCoachChatTestEnvironment(
  originalWebSocket: typeof WebSocket,
  originalLocation: Location,
) {
  vi.clearAllMocks();
  FakeWebSocket.instances = [];
  FakeWebSocket.failNextConnection = false;
  global.WebSocket = originalWebSocket;
  Object.defineProperty(window, 'location', {
    configurable: true,
    value: originalLocation,
  });
}
