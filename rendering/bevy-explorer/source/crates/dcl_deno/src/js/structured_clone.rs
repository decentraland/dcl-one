use deno_core::{op2, v8, OpDecl};
use deno_error::JsErrorBox;
use v8::{ValueDeserializerHelper, ValueSerializerHelper};

/// deno_core's `op_structured_clone`, without its host-object path.
pub fn override_ops() -> Vec<OpDecl> {
    vec![op_structured_clone()]
}

struct SceneClone;

/// Throws a TypeError, which the structured clone script turns into a DataCloneError.
fn refuse_clone(scope: &mut v8::PinScope, what: &str) {
    if let Some(message) = v8::String::new(scope, &format!("#<{what}> could not be cloned.")) {
        let error = v8::Exception::type_error(scope, message);
        scope.throw_exception(error);
    }
}

impl v8::ValueSerializerImpl for SceneClone {
    fn throw_data_clone_error<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        message: v8::Local<'s, v8::String>,
    ) {
        let error = v8::Exception::type_error(scope, message);
        scope.throw_exception(error);
    }

    fn write_host_object<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        _object: v8::Local<'s, v8::Object>,
        _value_serializer: &dyn v8::ValueSerializerHelper,
    ) -> Option<bool> {
        refuse_clone(scope, "Object");
        None
    }
}

impl v8::ValueDeserializerImpl for SceneClone {}

// Same name and arguments as the original; the brand and deserializers are ignored.
#[op2(reentrant)]
fn op_structured_clone<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: v8::Local<'s, v8::Value>,
    _deserializers: Option<v8::Local<'s, v8::Object>>,
) -> Result<v8::Local<'s, v8::Value>, JsErrorBox> {
    let serializer = v8::ValueSerializer::new(scope, Box::new(SceneClone));
    serializer.write_header();
    let written = {
        v8::tc_scope!(let scope, scope);
        let written = serializer.write_value(scope.get_current_context(), value);
        if scope.has_caught() || scope.has_terminated() {
            scope.rethrow();
            return Ok(v8::undefined(scope).into());
        }
        written
    };
    if written != Some(true) {
        return Err(JsErrorBox::type_error("Failed to serialize response"));
    }
    let bytes = serializer.release();
    let deserializer = v8::ValueDeserializer::new(scope, Box::new(SceneClone), &bytes);
    let context = scope.get_current_context();
    if !deserializer.read_header(context).unwrap_or_default() {
        return Err(JsErrorBox::type_error("could not deserialize value"));
    }
    deserializer
        .read_value(context)
        .ok_or_else(|| JsErrorBox::type_error("could not deserialize value"))
}
