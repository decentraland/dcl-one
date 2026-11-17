export type EventType = string | symbol;

export type Handler<T = unknown> = (event: T) => void;
export type WildcardHandler<T = Record<string, unknown>> = (
  type: keyof T,
  event: T[keyof T],
) => void;

export type EventHandlerList<T = unknown> = Array<Handler<T>>;
export type WildCardEventHandlerList<T = Record<string, unknown>> = Array<WildcardHandler<T>>;

export type EventHandlerMap<Events extends Record<EventType, unknown>> = Map<
  keyof Events | '*',
  EventHandlerList<Events[keyof Events]> | WildCardEventHandlerList<Events>
>;

export interface Emitter<Events extends Record<EventType, unknown>> {
  all: EventHandlerMap<Events>;

  on<Key extends keyof Events>(type: Key, handler: Handler<Events[Key]>): void;
  on(type: '*', handler: WildcardHandler<Events>): void;

  off<Key extends keyof Events>(type: Key, handler?: Handler<Events[Key]>): void;
  off(type: '*', handler: WildcardHandler<Events>): void;

  emit<Key extends keyof Events>(type: Key, event: Events[Key]): void;
  emit<Key extends keyof Events>(type: undefined extends Events[Key] ? Key : never): void;
}

/**
 * Makes an event emitter. `on` adds a handler for one type of event, or for all types with
 * `'*'`. `off` removes one handler, or all the handlers of the type when it gets no handler.
 * `emit` calls the handlers of the type in the order they were added, then the `'*'` handlers.
 * A handler that is added or removed during an `emit` does not change that `emit`.
 */
export function createEmitter<Events extends Record<EventType, unknown>>(
  all: EventHandlerMap<Events> = new Map(),
): Emitter<Events> {
  type AnyHandler = Handler<Events[keyof Events]> | WildcardHandler<Events>;

  return {
    all,

    on(type: keyof Events | '*', handler: AnyHandler) {
      const handlers = all.get(type) as AnyHandler[] | undefined;
      if (handlers) {
        handlers.push(handler);
      } else {
        all.set(type, [handler] as EventHandlerList<Events[keyof Events]>);
      }
    },

    off(type: keyof Events | '*', handler?: AnyHandler) {
      const handlers = all.get(type) as AnyHandler[] | undefined;
      if (!handlers) return;
      if (handler) {
        const index = handlers.indexOf(handler);
        if (index !== -1) handlers.splice(index, 1);
      } else {
        all.set(type, []);
      }
    },

    emit(type: keyof Events, event?: Events[keyof Events]) {
      const handlers = all.get(type) as EventHandlerList<Events[keyof Events]> | undefined;
      for (const handler of handlers?.slice() ?? []) handler(event!);
      const wildcard = all.get('*') as WildCardEventHandlerList<Events> | undefined;
      for (const handler of wildcard?.slice() ?? []) handler(type, event!);
    },
  };
}
