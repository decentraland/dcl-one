# 签名获取

该 `SignedFetch` 模块提供了对……的实现 `fetch` 与……透明兼容的接口 [签名获取](/docs/contributor/contributor-zh/shen-fen-yan-zheng/signed-fetch/) 协议，该协议定义了如何附加已签名 [认证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/) 到外发请求中。

对该请求进行签名和验证的流程详见 [身份验证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/) 页面。

使用 `SignedFetch` 从场景中使用需要 [`USE_FETCH`](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions) 权限。

### 方法

**`signedFetch`**

像使用（[`fetch`](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/globals/#http)）时一样发起 HTTP 请求，并自动添加验证头。

```ts
interface Request {
  // 请求目标 URL：
  url: string;

  // 请求的可选、自说明参数：
  init?: {
    method?: string;
    body?: string;
    headers: { [key: string]: string };
  };
}

interface Response {
  // HTTP 请求是否成功完成（非 2xx 状态码不视为失败）
  ok: boolean;

  // 响应的自说明细节：
  status: number;
  statusText: string;
  headers: { [key: string]: string };
  body: string;
}

function signedFetch(Request): Promise<Response>;
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/signed-fetch.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
