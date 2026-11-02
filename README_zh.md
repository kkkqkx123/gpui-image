# gpui-image

面向 [GPUI](https://gpui.rs) 应用的图像解码、处理与显示能力。

本 crate 补齐 GPUI `img()` 原语与业务需求之间的缺口：任意字节或文件可被解码、
变换（缩放 / 裁剪 / 旋转 / 翻转 / 滤镜 / 缩略图 / 格式转换），再以 `ImageSource`
交还 GPUI 渲染，或经通用 `Image` / `ImageFallback` 组件直接显示。

## 分层

| Crate | 内容 | GPUI 依赖 |
| ----- | ---- | --------- |
| `gpui-image-core` | 解码 / 编码 + 处理原语（纯逻辑） | 无 |
| `gpui-image-gpui` | `ImageSource` 构造（`still_image`）、`EncodedImage` 转换、`ImageCache` | 二选一 flavor |
| `gpui-image-component` | 通用 `Image` / `ImageFallback` 组件（`Sizable` / `Styled`） | 经由 `gpui-component` |

## GPUI flavor

二选一：`gpui-kit`（GPUI Kit 的 `gpui-pre` 类型，默认）或 `gpui-fast`。
所选 flavor 必须与应用本身构建的 GPUI 一致，因为桥接层会把 GPUI 类型
（`ImageSource`、`ObjectFit`）交还给应用。

`gpui-pre` / `gpui-fast` 的 pin 始终跟随最新已发布的 GPUI Kit，
`script/check-gpui-kit-pin.py` 在 CI 中校验对齐情况。

## 仓库形态

本仓库是独立仓库，经 `.gitmodules` 挂在 `gpui-kit` 的 `crates/gpui-image`
（与 `gpui-tray`、`gpui-autostart` 同一模式）。它自带 `[workspace]` 与
`Cargo.lock`，不是宿主 workspace 的成员，且从不修改 `gpui-base`——因此宿主
与上游同步只是一次 submodule 指针前移，不存在文本冲突。

## 示例

```rust,ignore
use gpui_image::core::{EncodedFormat, Filter, ImageBuffer};

let buffer = ImageBuffer::decode(&bytes)?
    .thumbnail(512)
    .filter(Filter::unsharp(1.0, 5));
let encoded = buffer.to_encoded(EncodedFormat::Png)?;
let source = gpui_image::bridge::ImageSource::from(encoded);
```

可运行应用见 `examples/image_demo`，完整设计（中文）见宿主仓库的
`docs/plan/image-processing-display.md`。
