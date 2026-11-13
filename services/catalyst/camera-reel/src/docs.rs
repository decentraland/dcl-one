//! `GET /api/docs/openapi.json` and the Swagger page at `/api/docs/ui/`
//! (and any path under it); the spec is the upstream camera-reel-service
//! one, the page loads its JSON from the sibling path so it works under any
//! mount prefix.

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use serde_json::{Value, json};

pub async fn openapi_json() -> Response {
    let spec = openapi_spec();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        Json(spec),
    )
        .into_response()
}

pub async fn swagger_ui() -> Html<String> {
    Html(SWAGGER_UI_HTML.to_string())
}

const SWAGGER_UI_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Camera Reel Service - Swagger UI</title>
<link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist/swagger-ui.css">
</head>
<body>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist/swagger-ui-bundle.js" crossorigin></script>
<script>
window.onload = function() {
  window.ui = SwaggerUIBundle({
    url: location.pathname.replace(/\/docs\/ui.*$/, "/docs/openapi.json"),
    dom_id: "#swagger-ui",
    deepLinking: true,
    presets: [SwaggerUIBundle.presets.apis]
  });
};
</script>
</body>
</html>"##;

fn ref_to(name: &str) -> Value {
    json!({ "$ref": format!("#/components/schemas/{name}") })
}

fn openapi_spec() -> Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Camera Reel Service",
            "description": "Camera Reel API",
            "license": { "name": "" },
            "version": env!("CARGO_PKG_VERSION")
        },
        "paths": {
            "/api/images": {
                "post": {
                    "tags": ["images"],
                    "operationId": "upload_image",
                    "requestBody": {
                        "description": "Image file and metadata in JSON format.",
                        "content": {
                            "multipart/form-data": { "schema": ref_to("Upload") }
                        },
                        "required": true
                    },
                    "responses": {
                        "200": { "description": "Uploaded image with its metadata", "content": { "application/json": { "schema": ref_to("UploadResponse") } } },
                        "400": { "description": "Bad Request", "content": { "application/json": { "schema": ref_to("ResponseError") } } },
                        "403": { "description": "Forbidden", "content": { "application/json": { "schema": ref_to("ForbiddenError") } } },
                        "500": { "description": "Internal Server Error", "content": { "application/json": { "schema": ref_to("ResponseError") } } }
                    }
                }
            },
            "/api/images/{image_id}": {
                "get": {
                    "tags": ["images"],
                    "operationId": "get_image",
                    "parameters": [{ "name": "image_id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": {
                        "200": { "description": "Get image", "content": { "application/json": { "schema": ref_to("Image") } } }
                    }
                },
                "delete": {
                    "tags": ["images"],
                    "operationId": "delete_image",
                    "parameters": [{ "name": "image_id", "in": "path", "description": "Image database id to delete", "required": true, "schema": { "type": "string" } }],
                    "responses": {
                        "200": { "description": "Image deleted", "content": { "application/json": { "schema": ref_to("UserDataResponse") } } },
                        "403": { "description": "Forbidden" },
                        "404": { "description": "Image was not found" },
                        "500": { "description": "Failed to delete image" }
                    }
                }
            },
            "/api/images/{image_id}/metadata": {
                "get": {
                    "tags": ["images"],
                    "operationId": "get_metadata",
                    "parameters": [{ "name": "image_id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": {
                        "200": { "description": "Get image metadata", "content": { "application/json": { "schema": ref_to("Image") } } },
                        "404": { "description": "Not found" }
                    }
                }
            },
            "/api/images/{id}/visibility": {
                "patch": {
                    "tags": ["images"],
                    "operationId": "update_image_visibility",
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "requestBody": {
                        "description": "Update image visibility",
                        "content": { "application/json": { "schema": ref_to("UpdateVisibility") } },
                        "required": true
                    },
                    "responses": {
                        "200": { "description": "Image visibility updated successfully" },
                        "403": { "description": "Forbidden" },
                        "404": { "description": "Image was not found" },
                        "500": { "description": "Failed to update image visibility" }
                    }
                }
            },
            "/api/users/{user_address}": {
                "get": {
                    "tags": ["images"],
                    "operationId": "get_user_data",
                    "parameters": [{ "name": "user_address", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": {
                        "200": { "description": "Get user data", "content": { "application/json": { "schema": ref_to("UserDataResponse") } } },
                        "404": { "description": "Not found" }
                    }
                }
            },
            "/api/users/{user_address}/images": {
                "get": {
                    "tags": ["images"],
                    "operationId": "get_user_images",
                    "parameters": [
                        { "name": "offset", "in": "query", "required": false, "schema": { "type": "integer", "format": "int64", "minimum": 0 } },
                        { "name": "limit", "in": "query", "required": false, "schema": { "type": "integer", "format": "int64", "minimum": 0 } },
                        { "name": "compact", "in": "query", "required": false, "schema": { "type": "boolean" } },
                        { "name": "user_address", "in": "path", "required": true, "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": { "description": "List images for a given user", "content": { "application/json": { "schema": ref_to("GetImagesResponse") } } },
                        "210": { "description": "List gallery images for a given user if `compact=true` (status code is 200, but was not possible to list multiple responses for one status code)", "content": { "application/json": { "schema": ref_to("GetGalleryImagesResponse") } } },
                        "404": { "description": "Not found" }
                    }
                }
            },
            "/api/places/{place_id}/images": {
                "get": {
                    "tags": ["images"],
                    "operationId": "get_place_images",
                    "parameters": [
                        { "name": "offset", "in": "query", "required": false, "schema": { "type": "integer", "format": "int64", "minimum": 0 } },
                        { "name": "limit", "in": "query", "required": false, "schema": { "type": "integer", "format": "int64", "minimum": 0 } },
                        { "name": "place_id", "in": "path", "required": true, "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": { "description": "List images for a given place", "content": { "application/json": { "schema": ref_to("GetPlaceImagesResponse") } } },
                        "404": { "description": "Not found" },
                        "502": { "description": "Failed to resolve world name" }
                    }
                }
            },
            "/api/places/images": {
                "post": {
                    "tags": ["images"],
                    "operationId": "get_multiple_places_images",
                    "parameters": [
                        { "name": "offset", "in": "query", "required": false, "schema": { "type": "integer", "format": "int64", "minimum": 0 } },
                        { "name": "limit", "in": "query", "required": false, "schema": { "type": "integer", "format": "int64", "minimum": 0 } }
                    ],
                    "requestBody": {
                        "description": "Object with a list of places IDs",
                        "content": { "application/json": { "schema": ref_to("GetMultiplePlacesImagesBody") } },
                        "required": true
                    },
                    "responses": {
                        "200": { "description": "List images for multiple places", "content": { "application/json": { "schema": ref_to("GetMultiplePlacesImagesResponse") } } },
                        "400": { "description": "Invalid place IDs format" },
                        "502": { "description": "Failed to resolve world name" }
                    }
                }
            },
            "/api/wearables/{item}/images": {
                "get": {
                    "tags": ["images"],
                    "operationId": "get_wearable_images",
                    "parameters": [
                        { "name": "offset", "in": "query", "required": false, "schema": { "type": "integer", "format": "int64", "minimum": 0 } },
                        { "name": "limit", "in": "query", "required": false, "schema": { "type": "integer", "format": "int64", "minimum": 0 } },
                        { "name": "item", "in": "path", "required": true, "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": { "description": "Public images of people wearing the item", "content": { "application/json": { "schema": ref_to("GetWearableImagesResponse") } } },
                        "400": { "description": "Malformed item id", "content": { "application/json": { "schema": ref_to("ResponseError") } } },
                        "500": { "description": "Internal Server Error", "content": { "application/json": { "schema": ref_to("ResponseError") } } }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "Location": {
                    "type": "object",
                    "required": ["x", "y"],
                    "properties": {
                        "x": { "type": "string" },
                        "y": { "type": "string" }
                    }
                },
                "Scene": {
                    "type": "object",
                    "required": ["name", "location"],
                    "properties": {
                        "name": { "type": "string" },
                        "location": ref_to("Location")
                    }
                },
                "User": {
                    "type": "object",
                    "required": ["userName", "userAddress", "wearables"],
                    "properties": {
                        "userName": { "type": "string" },
                        "userAddress": { "type": "string" },
                        "wearables": { "type": "array", "items": { "type": "string" } },
                        "isGuest": { "type": "boolean" },
                        "isEmoting": { "type": "boolean", "nullable": true },
                        "screenRect": ref_to("ScreenRect")
                    }
                },
                "ScreenRect": {
                    "type": "object",
                    "description": "Where a visible person stands in the photo, normalized to the image with the origin at its top-left corner. Every field is in 0..=1; x + width and y + height may exceed 1 by up to 1e-4.",
                    "required": ["x", "y", "width", "height"],
                    "properties": {
                        "x": { "type": "number", "format": "float" },
                        "y": { "type": "number", "format": "float" },
                        "width": { "type": "number", "format": "float" },
                        "height": { "type": "number", "format": "float" }
                    }
                },
                "Metadata": {
                    "type": "object",
                    "required": ["userName", "userAddress", "dateTime", "realm", "scene", "visiblePeople", "placeId"],
                    "properties": {
                        "userName": { "type": "string" },
                        "userAddress": { "type": "string" },
                        "dateTime": { "type": "string" },
                        "realm": { "type": "string" },
                        "scene": ref_to("Scene"),
                        "visiblePeople": { "type": "array", "items": ref_to("User") },
                        "placeId": { "type": "string" }
                    }
                },
                "Image": {
                    "type": "object",
                    "required": ["id", "url", "thumbnailUrl", "isPublic", "metadata"],
                    "properties": {
                        "id": { "type": "string" },
                        "url": { "type": "string" },
                        "thumbnailUrl": { "type": "string" },
                        "isPublic": { "type": "boolean" },
                        "metadata": ref_to("Metadata")
                    }
                },
                "GalleryImage": {
                    "type": "object",
                    "required": ["id", "url", "thumbnailUrl", "isPublic", "dateTime"],
                    "properties": {
                        "id": { "type": "string" },
                        "url": { "type": "string" },
                        "thumbnailUrl": { "type": "string" },
                        "isPublic": { "type": "boolean" },
                        "dateTime": { "type": "string" }
                    }
                },
                "GalleryImageWithPlace": {
                    "type": "object",
                    "required": ["id", "url", "thumbnailUrl", "isPublic", "dateTime", "placeId"],
                    "properties": {
                        "id": { "type": "string" },
                        "url": { "type": "string" },
                        "thumbnailUrl": { "type": "string" },
                        "isPublic": { "type": "boolean" },
                        "dateTime": { "type": "string" },
                        "placeId": { "type": "string" }
                    }
                },
                "Upload": {
                    "type": "object",
                    "required": ["image", "metadata", "is_public"],
                    "properties": {
                        "image": { "type": "string", "format": "binary" },
                        "is_public": { "type": "boolean" },
                        "metadata": { "type": "string", "format": "binary" }
                    }
                },
                "UploadResponse": {
                    "allOf": [
                        ref_to("UserDataResponse"),
                        {
                            "type": "object",
                            "required": ["image"],
                            "properties": { "image": ref_to("Image") }
                        }
                    ]
                },
                "UpdateVisibility": {
                    "type": "object",
                    "required": ["is_public"],
                    "properties": {
                        "is_public": { "type": "boolean" }
                    }
                },
                "UserDataResponse": {
                    "type": "object",
                    "required": ["currentImages", "maxImages"],
                    "properties": {
                        "currentImages": { "type": "integer", "format": "int64", "minimum": 0 },
                        "maxImages": { "type": "integer", "format": "int64", "minimum": 0 }
                    }
                },
                "PlaceDataResponse": {
                    "type": "object",
                    "required": ["maxImages"],
                    "properties": {
                        "maxImages": { "type": "integer", "format": "int64", "minimum": 0 }
                    }
                },
                "GetImagesResponse": {
                    "allOf": [
                        ref_to("UserDataResponse"),
                        {
                            "type": "object",
                            "required": ["images"],
                            "properties": { "images": { "type": "array", "items": ref_to("Image") } }
                        }
                    ]
                },
                "GetGalleryImagesResponse": {
                    "allOf": [
                        ref_to("UserDataResponse"),
                        {
                            "type": "object",
                            "required": ["images"],
                            "properties": { "images": { "type": "array", "items": ref_to("GalleryImage") } }
                        }
                    ]
                },
                "GetPlaceImagesResponse": {
                    "allOf": [
                        ref_to("PlaceDataResponse"),
                        {
                            "type": "object",
                            "required": ["images"],
                            "properties": { "images": { "type": "array", "items": ref_to("GalleryImage") } }
                        }
                    ]
                },
                "GetMultiplePlacesImagesBody": {
                    "type": "object",
                    "required": ["placesIds"],
                    "properties": {
                        "placesIds": { "type": "array", "items": { "type": "string" } }
                    }
                },
                "GetMultiplePlacesImagesResponse": {
                    "allOf": [
                        ref_to("PlaceDataResponse"),
                        {
                            "type": "object",
                            "required": ["images"],
                            "properties": { "images": { "type": "array", "items": ref_to("GalleryImageWithPlace") } }
                        }
                    ]
                },
                "GetWearableImagesResponse": {
                    "type": "object",
                    "required": ["images", "maxImages"],
                    "properties": {
                        "images": {
                            "type": "array",
                            "items": ref_to("Image"),
                            "description": "Full images rather than gallery rows: a consumer showing \"people wearing this\" needs who is in\nthe shot and where it was taken, and asking for the metadata of every photo it just received\nwould be one request per photo."
                        },
                        "maxImages": { "type": "integer", "format": "int64", "minimum": 0 }
                    }
                },
                "ForbiddenReason": {
                    "type": "string",
                    "enum": ["maxLimitReached"]
                },
                "ForbiddenError": {
                    "type": "object",
                    "required": ["reason", "message"],
                    "properties": {
                        "reason": ref_to("ForbiddenReason"),
                        "message": { "type": "string" }
                    }
                },
                "ResponseError": {
                    "type": "object",
                    "required": ["message"],
                    "properties": {
                        "message": { "type": "string" }
                    }
                }
            }
        },
        "tags": [
            { "name": "images", "description": "Images management endpoints." }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The production document (`docs-openapi`, 2026-09-25) with what
    /// upstream 3e8bd0b added after it (#61: `User.screenRect` and the
    /// `ScreenRect` component) and the one component it references but never
    /// defines (`GalleryImageWithPlace`).
    #[test]
    fn the_spec_is_the_captured_one_plus_the_later_upstream_screen_rect() {
        let path = format!(
            "{}/services/catalyst/tests/fixtures/conformance/social/camera-reel/docs-openapi.json",
            env!("DCL_ONE_ROOT")
        );
        let captured: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let spec = openapi_spec();
        let ours = &spec["components"]["schemas"];
        let mut expected = captured["response"]["body_json"].clone();
        expected["info"]["version"] = json!(env!("CARGO_PKG_VERSION"));
        let schemas = &mut expected["components"]["schemas"];
        assert!(schemas.get("GalleryImageWithPlace").is_none());
        schemas["GalleryImageWithPlace"] = ours["GalleryImageWithPlace"].clone();
        schemas["ScreenRect"] = ours["ScreenRect"].clone();
        schemas["User"]["properties"]["screenRect"] = ref_to("ScreenRect");
        assert_eq!(spec, expected);
    }

    #[test]
    fn the_spec_documents_the_wire_and_every_path_parameter() {
        let spec = openapi_spec();
        let schemas = &spec["components"]["schemas"];
        assert_eq!(
            schemas["UpdateVisibility"]["required"],
            json!(["is_public"])
        );
        assert_eq!(
            schemas["Upload"]["required"],
            json!(["image", "metadata", "is_public"])
        );
        assert_eq!(
            schemas["User"]["properties"]["screenRect"],
            ref_to("ScreenRect")
        );
        assert_eq!(
            schemas["ScreenRect"]["required"],
            json!(["x", "y", "width", "height"])
        );
        for (path, item) in spec["paths"].as_object().unwrap() {
            for (method, op) in item.as_object().unwrap() {
                let declared: Vec<&str> = op["parameters"]
                    .as_array()
                    .map(|ps| {
                        ps.iter()
                            .filter(|p| p["in"] == "path")
                            .map(|p| p["name"].as_str().unwrap())
                            .collect()
                    })
                    .unwrap_or_default();
                for segment in path.split('/') {
                    if let Some(name) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}'))
                    {
                        assert!(declared.contains(&name), "{method} {path} lacks {name}");
                    }
                }
            }
        }
    }
}
