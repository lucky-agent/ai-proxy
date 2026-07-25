use bytes::Bytes;
use rama::extensions::{Extension, ExtensionsRef};

/// 请求 body 缓存：ScriptLayer 收集后写入 extensions，
/// 下游（TrafficRecorderLayer / AiPipeline）直接读取，避免重复收集。
///
/// `Bytes` 为 Arc 引用计数，多读零拷贝。
#[derive(Clone, Debug, Extension)]
pub(crate) struct CachedRequestBody(pub Bytes);

/// 扩展 trait：从 rama 请求扩展中快速提取常用上下文。
pub(crate) trait RequestExt {
    /// 提取 `T` 类型扩展值，未找到时 panic。
    fn ext<T>(&self) -> T
    where
        T: Clone + Extension,
    {
        self.try_ext::<T>().unwrap_or_else(|| {
            panic!(
                "{} not found in request extensions",
                std::any::type_name::<T>()
            )
        })
    }

    /// 提取 `T` 类型扩展值，未找到时返回 `None`。
    fn try_ext<T: Extension + Clone>(&self) -> Option<T>;

    /// 检查扩展中是否存在 `T` 类型。
    fn has_ext<T: Extension>(&self) -> bool;
}

impl<E: ExtensionsRef> RequestExt for E {
    fn try_ext<T: Extension + Clone>(&self) -> Option<T> {
        self.extensions().get_ref::<T>().cloned()
    }

    fn has_ext<T: Extension>(&self) -> bool {
        self.extensions().get_ref::<T>().is_some()
    }
}
