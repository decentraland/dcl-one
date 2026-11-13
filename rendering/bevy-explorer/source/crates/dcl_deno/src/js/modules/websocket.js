// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Adapted from deno 1.46 ext/websocket/01_websocket.js: client role only, on this
// runtime's own ops.

import { core, primordials } from "ext:core/mod.js";
const {
  isAnyArrayBuffer,
  isArrayBuffer,
} = core;
import {
  op_ws_close,
  op_ws_connect,
  op_ws_create,
  op_ws_get_buffer,
  op_ws_get_buffer_as_string,
  op_ws_get_buffered_amount,
  op_ws_get_close_code,
  op_ws_get_error,
  op_ws_next_event,
  op_ws_send_binary,
  op_ws_send_binary_ab,
  op_ws_send_text,
} from "ext:core/ops";
const {
  ArrayBufferIsView,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeShift,
  ArrayPrototypeSome,
  ErrorPrototypeToString,
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  PromiseResolve,
  RegExpPrototypeExec,
  SafeSet,
  SetPrototypeGetSize,
  String,
  StringPrototypeEndsWith,
  StringPrototypeToLowerCase,
  Symbol,
  SymbolFor,
  SymbolIterator,
  TypedArrayPrototypeGetByteLength,
} = primordials;

const { URL } = core.loadExtScript("ext:deno_web/00_url.js");
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript("ext:deno_web/01_console.js");
const { HTTP_TOKEN_CODE_POINT_RE } = core.loadExtScript("ext:deno_web/00_infra.js");
const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");
const {
  CloseEvent,
  defineEventHandler,
  dispatch,
  ErrorEvent,
  Event,
  EventTarget,
  MessageEvent,
  setIsTrusted,
} = core.loadExtScript("ext:deno_web/02_event.js");
const { Blob, BlobPrototype } = core.loadExtScript("ext:deno_web/09_file.js");
const { getLocationHref } = core.loadExtScript("ext:deno_web/12_location.js");

webidl.converters["sequence<DOMString> or DOMString"] = (
  V,
  prefix,
  context,
  opts,
) => {
  // Union for (sequence<DOMString> or DOMString)
  if (webidl.type(V) === "Object" && V !== null) {
    if (V[SymbolIterator] !== undefined) {
      return webidl.converters["sequence<DOMString>"](V, prefix, context, opts);
    }
  }
  return webidl.converters.DOMString(V, prefix, context, opts);
};

webidl.converters["WebSocketSend"] = (V, prefix, context, opts) => {
  // Union for (Blob or ArrayBufferView or ArrayBuffer or USVString)
  if (ObjectPrototypeIsPrototypeOf(BlobPrototype, V)) {
    return webidl.converters["Blob"](V, prefix, context, opts);
  }
  if (typeof V === "object") {
    if (isAnyArrayBuffer(V)) {
      return webidl.converters["ArrayBuffer"](V, prefix, context, opts);
    }
    if (ArrayBufferIsView(V)) {
      return webidl.converters["ArrayBufferView"](V, prefix, context, opts);
    }
  }
  return webidl.converters["USVString"](V, prefix, context, opts);
};

/** state */
const CONNECTING = 0;
const OPEN = 1;
const CLOSING = 2;
const CLOSED = 3;

/** op_ws_next_event kinds */
const KIND_TEXT = 0;
const KIND_BINARY = 1;
const KIND_CLOSE = 2;

const CLOSE_ABNORMAL = 1006;

const _readyState = Symbol("[[readyState]]");
const _url = Symbol("[[url]]");
const _rid = Symbol("[[rid]]");
const _extensions = Symbol("[[extensions]]");
const _protocol = Symbol("[[protocol]]");
const _binaryType = Symbol("[[binaryType]]");
const _eventLoop = Symbol("[[eventLoop]]");
const _sendQueue = Symbol("[[sendQueue]]");
const _queueSend = Symbol("[[queueSend]]");
const _fail = Symbol("[[fail]]");
const _failSend = Symbol("[[failSend]]");
const _sendNow = Symbol("[[sendNow]]");
const _closedWhileConnecting = Symbol("[[closedWhileConnecting]]");

class WebSocket extends EventTarget {
  constructor(url, protocols = []) {
    super();
    this[webidl.brand] = webidl.brand;
    this[_rid] = undefined;
    this[_readyState] = CONNECTING;
    this[_extensions] = "";
    this[_protocol] = "";
    this[_url] = "";
    this[_binaryType] = "blob";
    this[_sendQueue] = [];
    this[_closedWhileConnecting] = false;

    const prefix = "Failed to construct 'WebSocket'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    url = webidl.converters.USVString(url, prefix, "Argument 1");
    protocols = webidl.converters["sequence<DOMString> or DOMString"](
      protocols,
      prefix,
      "Argument 2",
    );

    let wsURL;

    try {
      wsURL = new URL(url, getLocationHref());
    } catch (e) {
      throw new DOMException(e.message, "SyntaxError");
    }

    if (wsURL.protocol === "http:") {
      wsURL.protocol = "ws:";
    } else if (wsURL.protocol === "https:") {
      wsURL.protocol = "wss:";
    }

    if (wsURL.protocol !== "ws:" && wsURL.protocol !== "wss:") {
      throw new DOMException(
        "Only ws & wss schemes are allowed in a WebSocket URL.",
        "SyntaxError",
      );
    }

    if (wsURL.hash !== "" || StringPrototypeEndsWith(wsURL.href, "#")) {
      throw new DOMException(
        "Fragments are not allowed in a WebSocket URL.",
        "SyntaxError",
      );
    }

    this[_url] = wsURL.href;

    if (typeof protocols === "string") {
      protocols = [protocols];
    }

    if (
      protocols.length !==
        SetPrototypeGetSize(
          new SafeSet(
            ArrayPrototypeMap(protocols, (p) => StringPrototypeToLowerCase(p)),
          ),
        )
    ) {
      throw new DOMException(
        "Can't supply multiple times the same protocol.",
        "SyntaxError",
      );
    }

    if (
      ArrayPrototypeSome(
        protocols,
        (protocol) =>
          RegExpPrototypeExec(HTTP_TOKEN_CODE_POINT_RE, protocol) === null,
      )
    ) {
      throw new DOMException(
        "Invalid protocol value.",
        "SyntaxError",
      );
    }

    // A refused socket fails like an unreachable one: error and close events once the
    // constructor has returned.
    let rid;
    try {
      rid = op_ws_create(wsURL.href, ArrayPrototypeJoin(protocols, ", "));
    } catch (err) {
      this[_readyState] = CLOSED;
      PromisePrototypeThen(PromiseResolve(), () => this[_fail](err));
      return;
    }
    this[_rid] = rid;

    PromisePrototypeThen(
      op_ws_connect(rid),
      (protocol) => {
        if (this[_closedWhileConnecting]) {
          // close() ran after the connection completed but before this callback
          this[_readyState] = CLOSED;
          this[_fail](
            new DOMException(
              "WebSocket was closed before the connection was established",
              "NetworkError",
            ),
          );
          core.tryClose(rid);
          return;
        }
        this[_protocol] = protocol;
        if (this[_readyState] === CONNECTING) {
          this[_readyState] = OPEN;
          const event = new Event("open");
          this.dispatchEvent(event);
        }
        this[_eventLoop]();
      },
      (err) => {
        this[_readyState] = CLOSED;
        this[_fail](err);
        core.tryClose(rid);
      },
    );
  }

  get readyState() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_readyState];
  }

  get CONNECTING() {
    webidl.assertBranded(this, WebSocketPrototype);
    return CONNECTING;
  }
  get OPEN() {
    webidl.assertBranded(this, WebSocketPrototype);
    return OPEN;
  }
  get CLOSING() {
    webidl.assertBranded(this, WebSocketPrototype);
    return CLOSING;
  }
  get CLOSED() {
    webidl.assertBranded(this, WebSocketPrototype);
    return CLOSED;
  }

  get extensions() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_extensions];
  }

  get protocol() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_protocol];
  }

  get url() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_url];
  }

  get binaryType() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_binaryType];
  }
  set binaryType(value) {
    webidl.assertBranded(this, WebSocketPrototype);
    value = webidl.converters.DOMString(
      value,
      "Failed to set 'binaryType' on 'WebSocket'",
    );
    if (value === "blob" || value === "arraybuffer") {
      this[_binaryType] = value;
    }
  }

  get bufferedAmount() {
    webidl.assertBranded(this, WebSocketPrototype);
    if (this[_readyState] === OPEN || this[_readyState] === CLOSING) {
      try {
        return op_ws_get_buffered_amount(this[_rid]);
      } catch {
        return 0;
      }
    }
    return 0;
  }

  send(data) {
    webidl.assertBranded(this, WebSocketPrototype);
    const prefix = "Failed to execute 'send' on 'WebSocket'";

    webidl.requiredArguments(arguments.length, 1, prefix);
    data = webidl.converters.WebSocketSend(data, prefix, "Argument 1");

    if (this[_readyState] !== OPEN) {
      throw new DOMException("readyState not OPEN", "InvalidStateError");
    }

    if (this[_sendQueue].length === 0) {
      // Fast path if the send queue is empty, for example when only synchronous
      // data is being sent.
      if (ObjectPrototypeIsPrototypeOf(BlobPrototype, data)) {
        this[_queueSend](data);
      } else {
        try {
          this[_sendNow](data);
        } catch (err) {
          this[_failSend](err);
        }
      }
    } else {
      // Slower path if the send queue is not empty, for example when sending
      // asynchronous data like a Blob.
      this[_queueSend](data);
    }
  }

  close(code = undefined, reason = undefined) {
    webidl.assertBranded(this, WebSocketPrototype);
    const prefix = "Failed to execute 'close' on 'WebSocket'";

    if (code !== undefined) {
      code = webidl.converters["unsigned short"](code, prefix, "Argument 1", {
        clamp: true,
      });
    }

    if (reason !== undefined) {
      reason = webidl.converters.USVString(reason, prefix, "Argument 2");
    }

    if (
      code !== undefined &&
      !(code === 1000 || (3000 <= code && code < 5000))
    ) {
      throw new DOMException(
        "The close code must be either 1000 or in the range of 3000 to 4999.",
        "InvalidAccessError",
      );
    }

    if (
      reason !== undefined &&
      TypedArrayPrototypeGetByteLength(core.encode(reason)) > 123
    ) {
      throw new DOMException(
        "The close reason may not be longer than 123 bytes.",
        "SyntaxError",
      );
    }

    if (this[_readyState] === CONNECTING) {
      // cancels the handshake; the pending connect fails with error and close events
      this[_readyState] = CLOSING;
      this[_closedWhileConnecting] = true;
      try {
        op_ws_close(this[_rid], code, reason);
      } catch {
        // the connect rejection reports the failure
      }
    } else if (this[_readyState] === OPEN) {
      this[_readyState] = CLOSING;

      try {
        op_ws_close(this[_rid], code, reason);
      } catch (err) {
        this[_readyState] = CLOSED;
        this[_fail](err);
        core.tryClose(this[_rid]);
      }
    }
  }

  [_fail](err) {
    const errorEv = new ErrorEvent("error", {
      error: err,
      message: ErrorPrototypeToString(err),
    });
    this.dispatchEvent(errorEv);

    const closeEv = new CloseEvent("close", {
      wasClean: false,
      code: CLOSE_ABNORMAL,
    });
    this.dispatchEvent(closeEv);
  }

  // A send the runtime refused (full buffer) fails the socket instead of throwing.
  [_failSend](err) {
    if (this[_readyState] === CLOSED) {
      return;
    }
    this[_readyState] = CLOSED;
    this[_sendQueue].length = 0;
    core.tryClose(this[_rid]);
    PromisePrototypeThen(PromiseResolve(), () => this[_fail](err));
  }

  [_sendNow](data) {
    if (ArrayBufferIsView(data)) {
      op_ws_send_binary(this[_rid], data);
    } else if (isArrayBuffer(data)) {
      op_ws_send_binary_ab(this[_rid], data);
    } else {
      op_ws_send_text(this[_rid], String(data));
    }
  }

  async [_eventLoop]() {
    const rid = this[_rid];
    while (this[_readyState] !== CLOSED) {
      let kind;
      try {
        kind = await op_ws_next_event(rid);
      } catch (err) {
        if (this[_readyState] === CLOSED) {
          break;
        }
        this[_readyState] = CLOSED;
        this[_fail](err);
        core.tryClose(rid);
        break;
      }
      // the socket was failed on the scene side and has already reported it
      if (this[_readyState] === CLOSED) {
        break;
      }

      switch (kind) {
        case KIND_TEXT: {
          const data = op_ws_get_buffer_as_string(rid);
          if (data === undefined) {
            break;
          }

          const event = new MessageEvent("message", {
            data,
            origin: this[_url],
          });
          setIsTrusted(event, true);
          dispatch(this, event);
          break;
        }
        case KIND_BINARY: {
          const d = op_ws_get_buffer(rid);
          if (d == undefined) {
            break;
          }

          // deno-lint-ignore prefer-primordials
          const buffer = d.buffer;
          let data;
          if (this.binaryType === "blob") {
            data = new Blob([buffer]);
          } else {
            data = buffer;
          }

          const event = new MessageEvent("message", {
            data,
            origin: this[_url],
          });
          setIsTrusted(event, true);
          dispatch(this, event);
          break;
        }
        case KIND_CLOSE: {
          const code = op_ws_get_close_code(rid);
          const reason = op_ws_get_error(rid);
          this[_readyState] = CLOSED;

          const event = new CloseEvent("close", {
            wasClean: code !== CLOSE_ABNORMAL,
            code,
            reason,
          });
          this.dispatchEvent(event);
          core.tryClose(rid);
          break;
        }
        default: {
          /* error */
          this[_readyState] = CLOSED;

          const errorEv = new ErrorEvent("error", {
            message: op_ws_get_error(rid),
          });
          this.dispatchEvent(errorEv);

          const closeEv = new CloseEvent("close", {
            wasClean: false,
            code: CLOSE_ABNORMAL,
          });
          this.dispatchEvent(closeEv);
          core.tryClose(rid);
          break;
        }
      }
    }
  }

  async [_queueSend](data) {
    const queue = this[_sendQueue];

    ArrayPrototypePush(queue, data);

    if (queue.length > 1) {
      // There is already a send in progress, so we just push to the queue
      // and let that task handle sending of this data.
      return;
    }

    while (queue.length > 0) {
      let data = queue[0];
      try {
        if (ObjectPrototypeIsPrototypeOf(BlobPrototype, data)) {
          // deno-lint-ignore prefer-primordials
          data = await data.slice().arrayBuffer();
        }
        if (this[_readyState] !== OPEN) {
          // closed while a Blob was read: the rest is never sent
          queue.length = 0;
          return;
        }
        this[_sendNow](data);
      } catch (err) {
        this[_failSend](err);
        return;
      }
      ArrayPrototypeShift(queue);
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(WebSocketPrototype, this),
        keys: [
          "url",
          "readyState",
          "extensions",
          "protocol",
          "binaryType",
          "bufferedAmount",
          "onmessage",
          "onerror",
          "onclose",
          "onopen",
        ],
      }),
      inspectOptions,
    );
  }
}

ObjectDefineProperties(WebSocket, {
  CONNECTING: {
    value: 0,
  },
  OPEN: {
    value: 1,
  },
  CLOSING: {
    value: 2,
  },
  CLOSED: {
    value: 3,
  },
});

defineEventHandler(WebSocket.prototype, "message");
defineEventHandler(WebSocket.prototype, "error");
defineEventHandler(WebSocket.prototype, "close");
defineEventHandler(WebSocket.prototype, "open");

webidl.configureInterface(WebSocket);
const WebSocketPrototype = WebSocket.prototype;

export { WebSocket };
