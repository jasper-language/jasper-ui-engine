//! L1 host of native `ui.*` plugins: `.jaso` steps 1-9 in this process, using the shared `jaso` reader.
//! 原生 `ui.*` 插件的 L1 宿主：在本进程内执行 `.jaso` 第 1–9 步，读取器用共用的 `jaso` crate。
//!
//! A plugin runs inside the host process with no isolation: trust comes only from the signature
//! policy the caller supplies, and capabilities are declarations checked at load, not a sandbox.
//! 插件在宿主进程内运行，没有隔离：信任只来自调用方给出的签名策略，能力是装载时比对的声明，不是沙箱。

use crate::ui_plugin_gate::{
    admit_paint_output, relocation_value, ui_manifest_admit, UiManifestError, UiPaintCommandV1,
    UiPaintError, UiPaintRequestV1, UI_PAINT_COMMAND_CAPACITY, UI_PAINT_REQUEST_VERSION,
};
use jaso::code::ErrorCode;
pub use jaso::lifecycle::ModuleId as UiPluginId;
use jaso::lifecycle::{Phase, Registry, StopError, UnloadReply};
use jaso::platform::{FloatAbi, HostPlatform, PlatformMismatch, Target};
use jaso::trust::{Provenance, TrustPolicy};
use jaso::{Arch, HostV1, ImageRef, Jaso, ModuleKind, Stage};

/// Runtime version `core.runtime` is compared against.
/// 与 `core.runtime` 比较的运行时版本。
pub const UI_PLUGIN_RUNTIME_VERSION: (u32, u32, u32) = (0, 1, 0);
const FLOAT_ABI: FloatAbi = FloatAbi::Hard;
const AUDIT_LIMIT: usize = 256;
const LOG_LIMIT: usize = 1024;

/// The platform record a plugin must carry to load here, or None where this host cannot map code.
/// 插件在此装入必须带的平台记录；本宿主无法映射代码的平台为 None。
pub const fn host_platform() -> Option<HostPlatform<'static>> {
    // Other ISAs need instruction-cache maintenance that no test has exercised yet.
    // 其他指令集需要指令缓存维护，尚无测试覆盖。
    if cfg!(all(unix, target_arch = "x86_64")) {
        Some(HostPlatform {
            isa: Some(Arch::X86_64),
            isa_ext: &[],
            target: Target::Host,
            pointer_bytes: 8,
            float_abi: FLOAT_ABI,
            abi: jaso::ABI_C_V1,
        })
    } else {
        None
    }
}

/// Why a load failed; nothing stays mapped and the module table is unchanged.
/// 装载失败的原因；不留映射，模块表不变。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiPluginLoadError {
    /// A stable `.jaso` error code from steps 1-6a.
    /// 第 1–6a 步的 `.jaso` 稳定错误码。
    Jaso(ErrorCode),
    /// Step 3 failure with the differing field.
    /// 第 3 步失败及不一致的字段。
    Platform(PlatformMismatch),
    /// `ui.*` keys refused.
    /// `ui.*` 键被拒绝。
    Manifest(UiManifestError),
    /// This process cannot map plugin code.
    /// 本进程无法映射插件代码。
    UnsupportedHost,
    /// UI plugins are native runtime modules; Managed, Hybrid and build/boot files are refused.
    /// UI 插件是原生运行期模块；Managed、Hybrid 与 build/boot 阶段文件被拒绝。
    UnsupportedModule,
    /// A module of this name is already in the table, running or stopped.
    /// 表中已有同名模块（运行中或已停止）。
    DuplicateName,
    /// `core.host` names a table this host does not offer, or `jaso_init` is missing.
    /// `core.host` 指向本宿主未提供的表，或缺少 `jaso_init`。
    HostInterface,
    /// A relocation left the image.
    /// 重定位超出映像。
    Relocation,
    /// `mmap` or `mprotect` failed with this errno.
    /// `mmap` 或 `mprotect` 失败及其 errno。
    Map(i32),
    /// `jaso_init` or `jaso_start` returned nonzero.
    /// `jaso_init` 或 `jaso_start` 返回非 0。
    Hook(i32),
}

/// Why a paint call produced no commands.
/// 绘制调用没有产出命令的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiPluginPaintError {
    UnknownPlugin,
    /// The plugin was stopped; its entry is not called.
    /// 插件已停止；不调用其入口。
    NotRunning,
    Output(UiPaintError),
}

/// One audit record; development-mode admission of an unsigned module always leaves one.
/// 一条审计记录；开发模式放行未签名模块必定留下一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiPluginAudit {
    Loaded {
        name: String,
        publisher: Option<String>,
    },
    Rejected {
        error: UiPluginLoadError,
    },
}

type PaintEntry = unsafe extern "C" fn(*const UiPaintRequestV1, *mut UiPaintCommandV1, u32) -> i32;
type InitEntry = unsafe extern "C" fn(*const HostV1) -> i32;
type HookEntry = unsafe extern "C" fn() -> i32;

struct Mapping {
    base: *mut u8,
    len: usize,
}

impl Mapping {
    #[cfg(unix)]
    fn new(len: usize) -> Result<Mapping, UiPluginLoadError> {
        // SAFETY: a fresh anonymous private mapping aliases nothing.
        // SAFETY：新建的匿名私有映射不与任何内存别名。
        let base = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if base == libc::MAP_FAILED {
            return Err(UiPluginLoadError::Map(last_errno()));
        }
        Ok(Mapping {
            base: base.cast(),
            len,
        })
    }

    #[cfg(not(unix))]
    fn new(_len: usize) -> Result<Mapping, UiPluginLoadError> {
        Err(UiPluginLoadError::UnsupportedHost)
    }

    #[cfg(unix)]
    fn protect(&self, start: u32, end: u32, flags: u32) -> Result<(), UiPluginLoadError> {
        let mut protection = libc::PROT_NONE;
        if flags & jaso::FLAG_R != 0 {
            protection |= libc::PROT_READ;
        }
        if flags & jaso::FLAG_W != 0 {
            protection |= libc::PROT_WRITE;
        }
        if flags & jaso::FLAG_X != 0 {
            protection |= libc::PROT_EXEC;
        }
        // SAFETY: the range was checked against the mapping length and no references into it exist.
        // SAFETY：范围已对照映射长度检查，且不存在指向其中的引用。
        let done = unsafe {
            libc::mprotect(
                self.base.add(start as usize).cast(),
                (end - start) as usize,
                protection,
            )
        };
        if done != 0 {
            return Err(UiPluginLoadError::Map(last_errno()));
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn protect(&self, _start: u32, _end: u32, _flags: u32) -> Result<(), UiPluginLoadError> {
        Err(UiPluginLoadError::UnsupportedHost)
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: base and len came from a successful mmap; callers drop only after stopping.
        // SAFETY：base 与 len 来自成功的 mmap；调用方只在停止之后才释放。
        #[cfg(unix)]
        unsafe {
            libc::munmap(self.base.cast(), self.len);
        }
    }
}

#[cfg(unix)]
fn last_errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// Step 7 for one validated native image: copy, relocate, then apply W^X permissions.
/// 对一个已校验的原生镜像执行第 7 步：复制、重定位，再设置 W^X 权限。
fn map_image(image: &ImageRef<'_>) -> Result<Mapping, UiPluginLoadError> {
    let image_size = image.image_size;
    let mapping = Mapping::new(image_size as usize)?;
    for segment in image.segments() {
        let start = segment.vaddr as usize;
        let end = start
            .checked_add(segment.data.len())
            .ok_or(UiPluginLoadError::Relocation)?;
        if end > image_size as usize {
            return Err(UiPluginLoadError::Relocation);
        }
        // SAFETY: [start, end) is inside the writable mapping, and the file is a distinct allocation.
        // SAFETY：[start, end) 位于可写映射内，文件字节是另一块分配。
        unsafe {
            std::ptr::copy_nonoverlapping(
                segment.data.as_ptr(),
                mapping.base.add(start),
                segment.data.len(),
            );
        }
    }
    for reloc in image.relocs() {
        let value = relocation_value(mapping.base as u64, reloc.offset, reloc.addend, image_size)
            .ok_or(UiPluginLoadError::Relocation)?;
        // SAFETY: relocation_value proved offset + 8 <= image_size; the site may be unaligned in the file's view.
        // SAFETY：relocation_value 已确认 offset + 8 <= image_size；写入位置按未对齐处理。
        unsafe {
            mapping
                .base
                .add(reloc.offset as usize)
                .cast::<u64>()
                .write_unaligned(value);
        }
    }
    mapping.protect(0, image_size, 0)?;
    for segment in image.segments() {
        let (start, end) = segment.page_span();
        if end > image_size {
            return Err(UiPluginLoadError::Relocation);
        }
        mapping.protect(start, end, segment.flags)?;
    }
    Ok(mapping)
}

extern "C" fn host_log(pointer: *const u8, len: usize) {
    if pointer.is_null() {
        return;
    }
    // SAFETY: the host table's contract requires a readable range; the length is capped first.
    // SAFETY：宿主接口表约定该范围可读；长度先被限制。
    let bytes = unsafe { std::slice::from_raw_parts(pointer, len.min(LOG_LIMIT)) };
    eprintln!("ui plugin: {}", String::from_utf8_lossy(bytes));
}

extern "C" fn host_alloc(size: usize, align: usize) -> *mut u8 {
    match std::alloc::Layout::from_size_align(size, align) {
        // SAFETY: the layout has nonzero size.
        // SAFETY：布局大小非零。
        Ok(layout) if size != 0 => unsafe { std::alloc::alloc(layout) },
        _ => std::ptr::null_mut(),
    }
}

extern "C" fn host_dealloc(pointer: *mut u8, size: usize, align: usize) {
    if pointer.is_null() {
        return;
    }
    if let Ok(layout) = std::alloc::Layout::from_size_align(size, align) {
        // SAFETY: the host table's contract requires the arguments of the matching host_alloc.
        // SAFETY：宿主接口表约定参数与对应的 host_alloc 一致。
        unsafe { std::alloc::dealloc(pointer, layout) }
    }
}

static HOST_TABLE: HostV1 = HostV1 {
    version: jaso::HOST_TABLE_VERSION,
    size: std::mem::size_of::<HostV1>() as u32,
    log: host_log,
    alloc: host_alloc,
    dealloc: host_dealloc,
};

/// Steps 1-6a plus the `ui.*` gate. Maps and calls nothing, so any bytes are safe to pass.
/// 第 1–6a 步加 `ui.*` 门。不映射也不调用，因此任何字节都可安全传入。
pub fn admit<'a>(
    bytes: &'a [u8],
    policy: &TrustPolicy<'a>,
    platform: &HostPlatform<'_>,
    granted: &[&str],
    registry_satisfies: impl Fn(&jaso::manifest::Dependency<'_>) -> bool,
) -> Result<(Jaso<'a>, ImageRef<'a>, Provenance<'a>, &'a str), UiPluginLoadError> {
    let file = Jaso::parse(bytes).map_err(|e| UiPluginLoadError::Jaso(e.code()))?;
    let provenance = policy
        .verify(&file)
        .map_err(|e| UiPluginLoadError::Jaso(e.code()))?;
    let image = file.select(platform).map_err(UiPluginLoadError::Platform)?;
    if file
        .manifest
        .runtime
        .is_some_and(|range| !range.contains(UI_PLUGIN_RUNTIME_VERSION))
    {
        return Err(UiPluginLoadError::Jaso(ErrorCode::DependencyRuntime));
    }
    if !file
        .manifest
        .depends()
        .all(|dependency| registry_satisfies(&dependency))
    {
        return Err(UiPluginLoadError::Jaso(ErrorCode::DependencyUnsatisfied));
    }
    if file
        .manifest
        .requires()
        .any(|capability| !granted.contains(&capability))
    {
        return Err(UiPluginLoadError::Jaso(ErrorCode::CapabilityDenied));
    }
    jaso::binding::check_file(&file).map_err(|e| UiPluginLoadError::Jaso(e.code()))?;
    if file.kind != ModuleKind::Native || file.stage != Stage::Runtime {
        return Err(UiPluginLoadError::UnsupportedModule);
    }
    let entry = ui_manifest_admit(
        file.manifest.extension("ui.abi"),
        file.manifest.extension("ui.kind"),
        file.manifest.extension("ui.paint"),
        file.manifest.exports(),
        file.manifest.requires(),
    )
    .map_err(UiPluginLoadError::Manifest)?;
    Ok((file, image, provenance, entry))
}

struct Slot {
    paint: PaintEntry,
    stop: Option<HookEntry>,
    // Declared last: dropped only after the module was stopped.
    // 最后声明：只在模块停止之后才释放。
    _mapping: Mapping,
}

/// L1 module table of UI plugins and the mappings it owns; single-threaded by construction.
/// UI 插件的 L1 模块表及其持有的映射；构造上只能单线程使用。
#[derive(Default)]
pub struct UiPluginHost {
    registry: Registry,
    slots: Vec<Slot>,
    audit: Vec<UiPluginAudit>,
}

impl UiPluginHost {
    pub fn new() -> UiPluginHost {
        UiPluginHost::default()
    }

    /// Bounded audit trail, oldest first.
    /// 有界审计记录，最早的在前。
    pub fn audit(&self) -> &[UiPluginAudit] {
        &self.audit
    }

    fn record(&mut self, record: UiPluginAudit) {
        if self.audit.len() == AUDIT_LIMIT {
            self.audit.remove(0);
        }
        self.audit.push(record);
    }

    /// Runs the whole L1 load; all-or-nothing. Plugin code runs only after the policy admits the file.
    /// 执行完整 L1 装载，要么全成要么不留痕。插件代码只在策略放行文件后才运行。
    pub fn load(
        &mut self,
        bytes: &[u8],
        policy: &TrustPolicy<'_>,
        granted: &[&str],
    ) -> Result<UiPluginId, UiPluginLoadError> {
        let loaded = self.load_inner(bytes, policy, granted);
        match &loaded {
            Ok((_, record)) => self.record(record.clone()),
            Err(error) => self.record(UiPluginAudit::Rejected {
                error: error.clone(),
            }),
        }
        loaded.map(|(id, _)| id)
    }

    fn load_inner(
        &mut self,
        bytes: &[u8],
        policy: &TrustPolicy<'_>,
        granted: &[&str],
    ) -> Result<(UiPluginId, UiPluginAudit), UiPluginLoadError> {
        let platform = host_platform().ok_or(UiPluginLoadError::UnsupportedHost)?;
        let registry = &self.registry;
        let (file, image, provenance, entry) =
            admit(bytes, policy, &platform, granted, |dependency| {
                registry.satisfies(dependency)
            })?;
        if self.registry.contains(file.manifest.name) {
            return Err(UiPluginLoadError::DuplicateName);
        }
        let init = match file.manifest.host {
            Some(jaso::HOST_V1) => Some(
                export_offset(&image, jaso::INIT_SYMBOL).ok_or(UiPluginLoadError::HostInterface)?,
            ),
            Some(_) => return Err(UiPluginLoadError::HostInterface),
            None => None,
        };
        let paint = export_offset(&image, entry).ok_or(UiPluginLoadError::Manifest(
            UiManifestError::EntryNotExported,
        ))?;
        let start = export_offset(&image, jaso::START_SYMBOL);
        let stop = export_offset(&image, jaso::STOP_SYMBOL);

        let mapping = map_image(&image)?;
        // SAFETY: each offset is a declared function export inside an executable segment of this
        // image, checked by the reader; the signature policy is what vouches for the code behind it.
        // SAFETY：每个偏移都是读取器已查验的、位于本镜像可执行段内的已声明函数导出；其代码由签名策略担保。
        unsafe {
            let at = |offset: u32| mapping.base.add(offset as usize);
            if let Some(offset) = init {
                let init: InitEntry = std::mem::transmute(at(offset));
                let status = init(&HOST_TABLE);
                if status != 0 {
                    return Err(UiPluginLoadError::Hook(status));
                }
            }
            if let Some(offset) = start {
                let start: HookEntry = std::mem::transmute(at(offset));
                let status = start();
                if status != 0 {
                    return Err(UiPluginLoadError::Hook(status));
                }
            }
            let paint: PaintEntry = std::mem::transmute(at(paint));
            let stop = stop.map(|offset| std::mem::transmute::<*mut u8, HookEntry>(at(offset)));
            let id = self.registry.insert(&file.manifest);
            self.slots.push(Slot {
                paint,
                stop,
                _mapping: mapping,
            });
            let publisher = match provenance {
                Provenance::Signed { publisher } => Some(publisher.to_string()),
                Provenance::UnsignedDevelopment => None,
            };
            Ok((
                id,
                UiPluginAudit::Loaded {
                    name: file.manifest.name.to_string(),
                    publisher,
                },
            ))
        }
    }

    /// Calls the plugin's paint entry and returns its validated commands for one node.
    /// 调用插件的绘制入口，返回其对一个节点的已校验命令。
    pub fn paint(
        &mut self,
        id: UiPluginId,
        node_width: u32,
        node_height: u32,
    ) -> Result<Vec<UiPaintCommandV1>, UiPluginPaintError> {
        match self.registry.phase(id) {
            None => return Err(UiPluginPaintError::UnknownPlugin),
            Some(Phase::Stopped) => return Err(UiPluginPaintError::NotRunning),
            Some(Phase::Running) => {}
        }
        let slot = self
            .slots
            .get(id.0)
            .ok_or(UiPluginPaintError::UnknownPlugin)?;
        let request = UiPaintRequestV1 {
            version: UI_PAINT_REQUEST_VERSION,
            size: std::mem::size_of::<UiPaintRequestV1>() as u32,
            node_width,
            node_height,
        };
        let mut buffer = vec![UiPaintCommandV1::default(); UI_PAINT_COMMAND_CAPACITY as usize];
        // SAFETY: the entry was admitted at load; request and buffer outlive the call and the
        // capacity passed is the buffer's length.
        // SAFETY：入口已在装载时准入；请求与缓冲区活过这次调用，传入的容量等于缓冲区长度。
        let returned =
            unsafe { (slot.paint)(&request, buffer.as_mut_ptr(), UI_PAINT_COMMAND_CAPACITY) };
        admit_paint_output(returned, &buffer, node_width, node_height)
            .map(<[UiPaintCommandV1]>::to_vec)
            .map_err(UiPluginPaintError::Output)
    }

    /// Stops a plugin. Refused while a running module depends on it; `jaso_stop` runs at most once.
    /// 停止插件。有运行中的依赖者时拒绝；`jaso_stop` 至多运行一次。
    pub fn stop(&mut self, id: UiPluginId) -> Result<(), StopError> {
        self.registry.begin_stop(id)?;
        let stop = self.slots.get(id.0).and_then(|slot| slot.stop);
        if let Some(stop) = stop {
            // SAFETY: an admitted export of a mapped image; begin_stop made this the only call.
            // SAFETY：已映射镜像中已准入的导出；begin_stop 保证这是唯一一次调用。
            let status = unsafe { stop() };
            if status != 0 {
                return Err(StopError::Hook(status));
            }
        }
        Ok(())
    }

    /// Beta has no unload: `plugin` answers deferred, `system` refused, and both stay mapped.
    /// beta 没有卸载：`plugin` 答复推迟，`system` 答复拒绝，都保持映射。
    pub fn unload_request(&self, id: UiPluginId) -> Option<UnloadReply> {
        self.registry.unload_request(id)
    }
}

impl Drop for UiPluginHost {
    /// End-of-host teardown, not an unload request: stops in reverse load order, then unmaps.
    /// 宿主收尾，不是卸载请求：按装入逆序停止，再取消映射。
    fn drop(&mut self) {
        for index in (0..self.slots.len()).rev() {
            let _ = self.stop(UiPluginId(index));
        }
    }
}

fn export_offset(image: &ImageRef<'_>, name: &str) -> Option<u32> {
    image
        .export(name)
        .filter(|export| export.kind == jaso::EXPORT_FUNC)
        .map(|export| export.value)
}
