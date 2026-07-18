use rama::extensions::{Extension, ExtensionsRef};

/// 扩展 trait：从 rama 请求扩展中快速提取常用上下文。
pub(crate) trait RequestExt {
    /// 提取 `T` 类型扩展值，未找到时 panic。
    fn ext<T: Extension>(&self) -> T
    where
        T: Clone,
    {
        self.try_ext::<T>()
            .unwrap_or_else(|| panic!("{} not found in request extensions", std::any::type_name::<T>()))
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
