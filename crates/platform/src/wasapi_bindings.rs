#[inline]
pub unsafe fn PropVariantClear(pvar: *mut PROPVARIANT) -> windows_core::HRESULT {
    windows_core::link!("ole32.dll" "system" fn PropVariantClear(pvar : *mut PROPVARIANT) -> windows_core::HRESULT);
    unsafe { PropVariantClear(pvar) }
}
pub const AUDCLNT_BUFFERFLAGS_SILENT: _AUDCLNT_BUFFERFLAGS = 2;
pub const AUDCLNT_E_DEVICE_INVALIDATED: i32 = -2004287484;
pub const AUDCLNT_E_RESOURCES_INVALIDATED: i32 = -2004287450;
pub type AUDCLNT_SHAREMODE = i32;
pub const AUDCLNT_SHAREMODE_EXCLUSIVE: AUDCLNT_SHAREMODE = 1;
pub const AUDCLNT_SHAREMODE_SHARED: AUDCLNT_SHAREMODE = 0;
pub const AUDCLNT_STREAMFLAGS_EVENTCALLBACK: i32 = 262144;
pub const AUDCLNT_STREAMFLAGS_LOOPBACK: i32 = 131072;
pub const AUDCLNT_STREAMFLAGS_NOPERSIST: i32 = 524288;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BLOB {
    pub cbSize: u32,
    pub pBlobData: *mut u8,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BSTRBLOB {
    pub cbSize: u32,
    pub pData: *mut u8,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CABOOL {
    pub cElems: u32,
    pub pElems: *mut VARIANT_BOOL,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CABSTR {
    pub cElems: u32,
    pub pElems: *mut windows_core::BSTR,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CABSTRBLOB {
    pub cElems: u32,
    pub pElems: *mut BSTRBLOB,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAC {
    pub cElems: u32,
    pub pElems: *mut i8,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CACLIPDATA {
    pub cElems: u32,
    pub pElems: *mut CLIPDATA,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CACLSID {
    pub cElems: u32,
    pub pElems: *mut windows_core::GUID,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CACY {
    pub cElems: u32,
    pub pElems: *mut CY,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CADATE {
    pub cElems: u32,
    pub pElems: *mut f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CADBL {
    pub cElems: u32,
    pub pElems: *mut f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAFILETIME {
    pub cElems: u32,
    pub pElems: *mut FILETIME,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAFLT {
    pub cElems: u32,
    pub pElems: *mut f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAH {
    pub cElems: u32,
    pub pElems: *mut i64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAI {
    pub cElems: u32,
    pub pElems: *mut i16,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAL {
    pub cElems: u32,
    pub pElems: *mut i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CALPSTR {
    pub cElems: u32,
    pub pElems: *mut windows_core::PSTR,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CALPWSTR {
    pub cElems: u32,
    pub pElems: *mut windows_core::PWSTR,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAPROPVARIANT {
    pub cElems: u32,
    pub pElems: *mut PROPVARIANT,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CASCODE {
    pub cElems: u32,
    pub pElems: *mut SCODE,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAUB {
    pub cElems: u32,
    pub pElems: *mut u8,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAUH {
    pub cElems: u32,
    pub pElems: *mut u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAUI {
    pub cElems: u32,
    pub pElems: *mut u16,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CAUL {
    pub cElems: u32,
    pub pElems: *mut u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CLIPDATA {
    pub cbSize: u32,
    pub ulClipFmt: i32,
    pub pClipData: *mut u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union CY {
    pub Anonymous: CY_0,
    pub int64: i64,
}
impl Default for CY {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CY_0 {
    pub Lo: u32,
    pub Hi: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DECIMAL {
    pub wReserved: u16,
    pub Anonymous: DECIMAL_0,
    pub Hi32: u32,
    pub Anonymous2: DECIMAL_1,
}
impl Default for DECIMAL {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union DECIMAL_0 {
    pub Anonymous: DECIMAL_0_0,
    pub signscale: u16,
}
impl Default for DECIMAL_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DECIMAL_0_0 {
    pub scale: u8,
    pub sign: u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union DECIMAL_1 {
    pub Anonymous: DECIMAL_1_0,
    pub Lo64: u64,
}
impl Default for DECIMAL_1 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DECIMAL_1_0 {
    pub Lo32: u32,
    pub Mid32: u32,
}
pub const DEVICE_STATE_ACTIVE: i32 = 1;
pub type EDataFlow = i32;
pub const EDataFlow_enum_count: EDataFlow = 3;
pub type ERole = i32;
pub const ERole_enum_count: ERole = 3;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FILETIME {
    pub dwLowDateTime: u32,
    pub dwHighDateTime: u32,
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HANDLE(pub *mut core::ffi::c_void);
windows_core::imp::define_interface!(
    IAudioCaptureClient,
    IAudioCaptureClient_Vtbl,
    0xc8adbd64_e71e_48a0_a4de_185c395cd317
);
windows_core::imp::interface_hierarchy!(IAudioCaptureClient, windows_core::IUnknown);
impl IAudioCaptureClient {
    pub(crate) unsafe fn GetBuffer(
        &self,
        ppdata: *mut *mut u8,
        pnumframestoread: *mut u32,
        pdwflags: *mut u32,
        pu64deviceposition: Option<*mut u64>,
        pu64qpcposition: Option<*mut u64>,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetBuffer)(
                windows_core::Interface::as_raw(self),
                ppdata as _,
                pnumframestoread as _,
                pdwflags as _,
                pu64deviceposition.unwrap_or(core::mem::zeroed()) as _,
                pu64qpcposition.unwrap_or(core::mem::zeroed()) as _,
            )
        }
    }
    pub(crate) unsafe fn ReleaseBuffer(&self, numframesread: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).ReleaseBuffer)(
                windows_core::Interface::as_raw(self),
                numframesread,
            )
        }
    }
    pub(crate) unsafe fn GetNextPacketSize(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetNextPacketSize)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IAudioCaptureClient_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetBuffer: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut u8,
        *mut u32,
        *mut u32,
        *mut u64,
        *mut u64,
    ) -> windows_core::HRESULT,
    pub ReleaseBuffer:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub GetNextPacketSize:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
}
pub trait IAudioCaptureClient_Impl: windows_core::IUnknownImpl {
    fn GetBuffer(
        &self,
        ppdata: *mut *mut u8,
        pnumframestoread: *mut u32,
        pdwflags: *mut u32,
        pu64deviceposition: *mut u64,
        pu64qpcposition: *mut u64,
    ) -> windows_core::Result<()>;
    fn ReleaseBuffer(&self, numframesread: u32) -> windows_core::Result<()>;
    fn GetNextPacketSize(&self) -> windows_core::Result<u32>;
}
impl IAudioCaptureClient_Vtbl {
    pub const fn new<Identity: IAudioCaptureClient_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn GetBuffer<
            Identity: IAudioCaptureClient_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            ppdata: *mut *mut u8,
            pnumframestoread: *mut u32,
            pdwflags: *mut u32,
            pu64deviceposition: *mut u64,
            pu64qpcposition: *mut u64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioCaptureClient_Impl::GetBuffer(
                    this,
                    core::mem::transmute_copy(&ppdata),
                    core::mem::transmute_copy(&pnumframestoread),
                    core::mem::transmute_copy(&pdwflags),
                    core::mem::transmute_copy(&pu64deviceposition),
                    core::mem::transmute_copy(&pu64qpcposition),
                )
                .into()
            }
        }
        unsafe extern "system" fn ReleaseBuffer<
            Identity: IAudioCaptureClient_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            numframesread: u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioCaptureClient_Impl::ReleaseBuffer(
                    this,
                    core::mem::transmute_copy(&numframesread),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetNextPacketSize<
            Identity: IAudioCaptureClient_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pnumframesinnextpacket: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IAudioCaptureClient_Impl::GetNextPacketSize(this) {
                    Ok(ok__) => {
                        pnumframesinnextpacket.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            GetBuffer: GetBuffer::<Identity, OFFSET>,
            ReleaseBuffer: ReleaseBuffer::<Identity, OFFSET>,
            GetNextPacketSize: GetNextPacketSize::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IAudioCaptureClient as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IAudioCaptureClient {}
windows_core::imp::define_interface!(
    IAudioClient,
    IAudioClient_Vtbl,
    0x1cb9ad4c_dbfa_4c32_b178_c2f568a703b2
);
windows_core::imp::interface_hierarchy!(IAudioClient, windows_core::IUnknown);
impl IAudioClient {
    pub(crate) unsafe fn Initialize(
        &self,
        sharemode: AUDCLNT_SHAREMODE,
        streamflags: u32,
        hnsbufferduration: REFERENCE_TIME,
        hnsperiodicity: REFERENCE_TIME,
        pformat: *const WAVEFORMATEX,
        audiosessionguid: Option<*const windows_core::GUID>,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Initialize)(
                windows_core::Interface::as_raw(self),
                sharemode,
                streamflags,
                hnsbufferduration,
                hnsperiodicity,
                pformat,
                audiosessionguid.unwrap_or(core::mem::zeroed()) as _,
            )
        }
    }
    pub(crate) unsafe fn GetBufferSize(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetBufferSize)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn GetStreamLatency(&self) -> windows_core::Result<REFERENCE_TIME> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetStreamLatency)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn GetCurrentPadding(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetCurrentPadding)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn IsFormatSupported(
        &self,
        sharemode: AUDCLNT_SHAREMODE,
        pformat: *const WAVEFORMATEX,
        ppclosestmatch: Option<*mut *mut WAVEFORMATEX>,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).IsFormatSupported)(
                windows_core::Interface::as_raw(self),
                sharemode,
                pformat,
                ppclosestmatch.unwrap_or(core::mem::zeroed()) as _,
            )
        }
    }
    pub(crate) unsafe fn GetMixFormat(&self) -> windows_core::Result<*mut WAVEFORMATEX> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetMixFormat)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn GetDevicePeriod(
        &self,
        phnsdefaultdeviceperiod: Option<*mut REFERENCE_TIME>,
        phnsminimumdeviceperiod: Option<*mut REFERENCE_TIME>,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetDevicePeriod)(
                windows_core::Interface::as_raw(self),
                phnsdefaultdeviceperiod.unwrap_or(core::mem::zeroed()) as _,
                phnsminimumdeviceperiod.unwrap_or(core::mem::zeroed()) as _,
            )
        }
    }
    pub(crate) unsafe fn Start(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Start)(windows_core::Interface::as_raw(self))
        }
    }
    pub(crate) unsafe fn Stop(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Stop)(windows_core::Interface::as_raw(self))
        }
    }
    pub(crate) unsafe fn Reset(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Reset)(windows_core::Interface::as_raw(self))
        }
    }
    pub(crate) unsafe fn SetEventHandle(&self, eventhandle: HANDLE) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetEventHandle)(
                windows_core::Interface::as_raw(self),
                eventhandle,
            )
        }
    }
    pub(crate) unsafe fn GetService<T>(&self) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).GetService)(
                windows_core::Interface::as_raw(self),
                &T::IID,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IAudioClient_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Initialize: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        AUDCLNT_SHAREMODE,
        u32,
        REFERENCE_TIME,
        REFERENCE_TIME,
        *const WAVEFORMATEX,
        *const windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub GetBufferSize:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetStreamLatency: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut REFERENCE_TIME,
    ) -> windows_core::HRESULT,
    pub GetCurrentPadding:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub IsFormatSupported: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        AUDCLNT_SHAREMODE,
        *const WAVEFORMATEX,
        *mut *mut WAVEFORMATEX,
    ) -> windows_core::HRESULT,
    pub GetMixFormat: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut WAVEFORMATEX,
    ) -> windows_core::HRESULT,
    pub GetDevicePeriod: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut REFERENCE_TIME,
        *mut REFERENCE_TIME,
    ) -> windows_core::HRESULT,
    pub Start: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Stop: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Reset: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetEventHandle:
        unsafe extern "system" fn(*mut core::ffi::c_void, HANDLE) -> windows_core::HRESULT,
    pub GetService: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub trait IAudioClient_Impl: windows_core::IUnknownImpl {
    fn Initialize(
        &self,
        sharemode: AUDCLNT_SHAREMODE,
        streamflags: u32,
        hnsbufferduration: REFERENCE_TIME,
        hnsperiodicity: REFERENCE_TIME,
        pformat: *const WAVEFORMATEX,
        audiosessionguid: *const windows_core::GUID,
    ) -> windows_core::Result<()>;
    fn GetBufferSize(&self) -> windows_core::Result<u32>;
    fn GetStreamLatency(&self) -> windows_core::Result<REFERENCE_TIME>;
    fn GetCurrentPadding(&self) -> windows_core::Result<u32>;
    fn IsFormatSupported(
        &self,
        sharemode: AUDCLNT_SHAREMODE,
        pformat: *const WAVEFORMATEX,
        ppclosestmatch: *mut *mut WAVEFORMATEX,
    ) -> windows_core::Result<()>;
    fn GetMixFormat(&self) -> windows_core::Result<*mut WAVEFORMATEX>;
    fn GetDevicePeriod(
        &self,
        phnsdefaultdeviceperiod: *mut REFERENCE_TIME,
        phnsminimumdeviceperiod: *mut REFERENCE_TIME,
    ) -> windows_core::Result<()>;
    fn Start(&self) -> windows_core::Result<()>;
    fn Stop(&self) -> windows_core::Result<()>;
    fn Reset(&self) -> windows_core::Result<()>;
    fn SetEventHandle(&self, eventhandle: HANDLE) -> windows_core::Result<()>;
    fn GetService(
        &self,
        riid: *const windows_core::GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()>;
}
impl IAudioClient_Vtbl {
    pub const fn new<Identity: IAudioClient_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Initialize<Identity: IAudioClient_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            sharemode: AUDCLNT_SHAREMODE,
            streamflags: u32,
            hnsbufferduration: REFERENCE_TIME,
            hnsperiodicity: REFERENCE_TIME,
            pformat: *const WAVEFORMATEX,
            audiosessionguid: *const windows_core::GUID,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioClient_Impl::Initialize(
                    this,
                    core::mem::transmute_copy(&sharemode),
                    core::mem::transmute_copy(&streamflags),
                    core::mem::transmute_copy(&hnsbufferduration),
                    core::mem::transmute_copy(&hnsperiodicity),
                    core::mem::transmute_copy(&pformat),
                    core::mem::transmute_copy(&audiosessionguid),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetBufferSize<
            Identity: IAudioClient_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pnumbufferframes: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IAudioClient_Impl::GetBufferSize(this) {
                    Ok(ok__) => {
                        pnumbufferframes.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetStreamLatency<
            Identity: IAudioClient_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            phnslatency: *mut REFERENCE_TIME,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IAudioClient_Impl::GetStreamLatency(this) {
                    Ok(ok__) => {
                        phnslatency.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetCurrentPadding<
            Identity: IAudioClient_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pnumpaddingframes: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IAudioClient_Impl::GetCurrentPadding(this) {
                    Ok(ok__) => {
                        pnumpaddingframes.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn IsFormatSupported<
            Identity: IAudioClient_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            sharemode: AUDCLNT_SHAREMODE,
            pformat: *const WAVEFORMATEX,
            ppclosestmatch: *mut *mut WAVEFORMATEX,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioClient_Impl::IsFormatSupported(
                    this,
                    core::mem::transmute_copy(&sharemode),
                    core::mem::transmute_copy(&pformat),
                    core::mem::transmute_copy(&ppclosestmatch),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetMixFormat<Identity: IAudioClient_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            ppdeviceformat: *mut *mut WAVEFORMATEX,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IAudioClient_Impl::GetMixFormat(this) {
                    Ok(ok__) => {
                        ppdeviceformat.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetDevicePeriod<
            Identity: IAudioClient_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            phnsdefaultdeviceperiod: *mut REFERENCE_TIME,
            phnsminimumdeviceperiod: *mut REFERENCE_TIME,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioClient_Impl::GetDevicePeriod(
                    this,
                    core::mem::transmute_copy(&phnsdefaultdeviceperiod),
                    core::mem::transmute_copy(&phnsminimumdeviceperiod),
                )
                .into()
            }
        }
        unsafe extern "system" fn Start<Identity: IAudioClient_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioClient_Impl::Start(this).into()
            }
        }
        unsafe extern "system" fn Stop<Identity: IAudioClient_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioClient_Impl::Stop(this).into()
            }
        }
        unsafe extern "system" fn Reset<Identity: IAudioClient_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioClient_Impl::Reset(this).into()
            }
        }
        unsafe extern "system" fn SetEventHandle<
            Identity: IAudioClient_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            eventhandle: HANDLE,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioClient_Impl::SetEventHandle(this, core::mem::transmute_copy(&eventhandle))
                    .into()
            }
        }
        unsafe extern "system" fn GetService<Identity: IAudioClient_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            riid: *const windows_core::GUID,
            ppv: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IAudioClient_Impl::GetService(
                    this,
                    core::mem::transmute_copy(&riid),
                    core::mem::transmute_copy(&ppv),
                )
                .into()
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            Initialize: Initialize::<Identity, OFFSET>,
            GetBufferSize: GetBufferSize::<Identity, OFFSET>,
            GetStreamLatency: GetStreamLatency::<Identity, OFFSET>,
            GetCurrentPadding: GetCurrentPadding::<Identity, OFFSET>,
            IsFormatSupported: IsFormatSupported::<Identity, OFFSET>,
            GetMixFormat: GetMixFormat::<Identity, OFFSET>,
            GetDevicePeriod: GetDevicePeriod::<Identity, OFFSET>,
            Start: Start::<Identity, OFFSET>,
            Stop: Stop::<Identity, OFFSET>,
            Reset: Reset::<Identity, OFFSET>,
            SetEventHandle: SetEventHandle::<Identity, OFFSET>,
            GetService: GetService::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IAudioClient as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IAudioClient {}
windows_core::imp::define_interface!(
    IDispatch,
    IDispatch_Vtbl,
    0x00020400_0000_0000_c000_000000000046
);
windows_core::imp::interface_hierarchy!(IDispatch, windows_core::IUnknown);
#[repr(C)]
pub struct IDispatch_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetTypeInfoCount: usize,
    GetTypeInfo: usize,
    GetIDsOfNames: usize,
    Invoke: usize,
}
impl windows_core::RuntimeName for IDispatch {}
windows_core::imp::define_interface!(
    IMMDevice,
    IMMDevice_Vtbl,
    0xd666063f_1587_4e43_81f1_b948e807363f
);
windows_core::imp::interface_hierarchy!(IMMDevice, windows_core::IUnknown);
impl IMMDevice {
    pub(crate) unsafe fn Activate<T>(
        &self,
        dwclsctx: u32,
        pactivationparams: Option<*const PROPVARIANT>,
    ) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).Activate)(
                windows_core::Interface::as_raw(self),
                &T::IID,
                dwclsctx,
                pactivationparams.unwrap_or(core::mem::zeroed()) as _,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn OpenPropertyStore(
        &self,
        stgmaccess: u32,
    ) -> windows_core::Result<IPropertyStore> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).OpenPropertyStore)(
                windows_core::Interface::as_raw(self),
                stgmaccess,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetId(&self) -> windows_core::Result<windows_core::PWSTR> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetId)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn GetState(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetState)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IMMDevice_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Activate: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        u32,
        *const PROPVARIANT,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub OpenPropertyStore: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetId: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::PWSTR,
    ) -> windows_core::HRESULT,
    pub GetState:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
}
pub trait IMMDevice_Impl: windows_core::IUnknownImpl {
    fn Activate(
        &self,
        iid: *const windows_core::GUID,
        dwclsctx: u32,
        pactivationparams: *const PROPVARIANT,
        ppinterface: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()>;
    fn OpenPropertyStore(&self, stgmaccess: u32) -> windows_core::Result<IPropertyStore>;
    fn GetId(&self) -> windows_core::Result<windows_core::PWSTR>;
    fn GetState(&self) -> windows_core::Result<u32>;
}
impl IMMDevice_Vtbl {
    pub const fn new<Identity: IMMDevice_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Activate<Identity: IMMDevice_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            iid: *const windows_core::GUID,
            dwclsctx: u32,
            pactivationparams: *const PROPVARIANT,
            ppinterface: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IMMDevice_Impl::Activate(
                    this,
                    core::mem::transmute_copy(&iid),
                    core::mem::transmute_copy(&dwclsctx),
                    core::mem::transmute_copy(&pactivationparams),
                    core::mem::transmute_copy(&ppinterface),
                )
                .into()
            }
        }
        unsafe extern "system" fn OpenPropertyStore<
            Identity: IMMDevice_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            stgmaccess: u32,
            ppproperties: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IMMDevice_Impl::OpenPropertyStore(
                    this,
                    core::mem::transmute_copy(&stgmaccess),
                ) {
                    Ok(ok__) => {
                        ppproperties.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetId<Identity: IMMDevice_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            ppstrid: *mut windows_core::PWSTR,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IMMDevice_Impl::GetId(this) {
                    Ok(ok__) => {
                        ppstrid.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetState<Identity: IMMDevice_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            pdwstate: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IMMDevice_Impl::GetState(this) {
                    Ok(ok__) => {
                        pdwstate.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            Activate: Activate::<Identity, OFFSET>,
            OpenPropertyStore: OpenPropertyStore::<Identity, OFFSET>,
            GetId: GetId::<Identity, OFFSET>,
            GetState: GetState::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IMMDevice as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IMMDevice {}
windows_core::imp::define_interface!(
    IMMDeviceCollection,
    IMMDeviceCollection_Vtbl,
    0x0bd7a1be_7a1a_44db_8397_cc5392387b5e
);
windows_core::imp::interface_hierarchy!(IMMDeviceCollection, windows_core::IUnknown);
#[repr(C)]
pub struct IMMDeviceCollection_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetCount: usize,
    Item: usize,
}
impl windows_core::RuntimeName for IMMDeviceCollection {}
windows_core::imp::define_interface!(
    IMMDeviceEnumerator,
    IMMDeviceEnumerator_Vtbl,
    0xa95664d2_9614_4f35_a746_de8db63617e6
);
windows_core::imp::interface_hierarchy!(IMMDeviceEnumerator, windows_core::IUnknown);
impl IMMDeviceEnumerator {
    pub(crate) unsafe fn EnumAudioEndpoints(
        &self,
        dataflow: EDataFlow,
        dwstatemask: u32,
    ) -> windows_core::Result<IMMDeviceCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumAudioEndpoints)(
                windows_core::Interface::as_raw(self),
                dataflow,
                dwstatemask,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetDefaultAudioEndpoint(
        &self,
        dataflow: EDataFlow,
        role: ERole,
    ) -> windows_core::Result<IMMDevice> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetDefaultAudioEndpoint)(
                windows_core::Interface::as_raw(self),
                dataflow,
                role,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetDevice<P0>(&self, pwstrid: P0) -> windows_core::Result<IMMDevice>
    where
        P0: windows_core::Param<windows_core::PCWSTR>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetDevice)(
                windows_core::Interface::as_raw(self),
                pwstrid.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn RegisterEndpointNotificationCallback<P0>(
        &self,
        pclient: P0,
    ) -> windows_core::HRESULT
    where
        P0: windows_core::Param<IMMNotificationClient>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).RegisterEndpointNotificationCallback)(
                windows_core::Interface::as_raw(self),
                pclient.param().abi(),
            )
        }
    }
    pub(crate) unsafe fn UnregisterEndpointNotificationCallback<P0>(
        &self,
        pclient: P0,
    ) -> windows_core::HRESULT
    where
        P0: windows_core::Param<IMMNotificationClient>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).UnregisterEndpointNotificationCallback)(
                windows_core::Interface::as_raw(self),
                pclient.param().abi(),
            )
        }
    }
}
#[repr(C)]
pub struct IMMDeviceEnumerator_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub EnumAudioEndpoints: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        EDataFlow,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetDefaultAudioEndpoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        EDataFlow,
        ERole,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::PCWSTR,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub RegisterEndpointNotificationCallback: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    )
        -> windows_core::HRESULT,
    pub UnregisterEndpointNotificationCallback: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    )
        -> windows_core::HRESULT,
}
pub trait IMMDeviceEnumerator_Impl: windows_core::IUnknownImpl {
    fn EnumAudioEndpoints(
        &self,
        dataflow: EDataFlow,
        dwstatemask: u32,
    ) -> windows_core::Result<IMMDeviceCollection>;
    fn GetDefaultAudioEndpoint(
        &self,
        dataflow: EDataFlow,
        role: ERole,
    ) -> windows_core::Result<IMMDevice>;
    fn GetDevice(&self, pwstrid: &windows_core::PCWSTR) -> windows_core::Result<IMMDevice>;
    fn RegisterEndpointNotificationCallback(
        &self,
        pclient: windows_core::Ref<IMMNotificationClient>,
    ) -> windows_core::Result<()>;
    fn UnregisterEndpointNotificationCallback(
        &self,
        pclient: windows_core::Ref<IMMNotificationClient>,
    ) -> windows_core::Result<()>;
}
impl IMMDeviceEnumerator_Vtbl {
    pub const fn new<Identity: IMMDeviceEnumerator_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn EnumAudioEndpoints<
            Identity: IMMDeviceEnumerator_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            dataflow: EDataFlow,
            dwstatemask: u32,
            ppdevices: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IMMDeviceEnumerator_Impl::EnumAudioEndpoints(
                    this,
                    core::mem::transmute_copy(&dataflow),
                    core::mem::transmute_copy(&dwstatemask),
                ) {
                    Ok(ok__) => {
                        ppdevices.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetDefaultAudioEndpoint<
            Identity: IMMDeviceEnumerator_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            dataflow: EDataFlow,
            role: ERole,
            ppendpoint: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IMMDeviceEnumerator_Impl::GetDefaultAudioEndpoint(
                    this,
                    core::mem::transmute_copy(&dataflow),
                    core::mem::transmute_copy(&role),
                ) {
                    Ok(ok__) => {
                        ppendpoint.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetDevice<
            Identity: IMMDeviceEnumerator_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pwstrid: windows_core::PCWSTR,
            ppdevice: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IMMDeviceEnumerator_Impl::GetDevice(this, core::mem::transmute(&pwstrid)) {
                    Ok(ok__) => {
                        ppdevice.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn RegisterEndpointNotificationCallback<
            Identity: IMMDeviceEnumerator_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pclient: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IMMDeviceEnumerator_Impl::RegisterEndpointNotificationCallback(
                    this,
                    core::mem::transmute_copy(&pclient),
                )
                .into()
            }
        }
        unsafe extern "system" fn UnregisterEndpointNotificationCallback<
            Identity: IMMDeviceEnumerator_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pclient: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IMMDeviceEnumerator_Impl::UnregisterEndpointNotificationCallback(
                    this,
                    core::mem::transmute_copy(&pclient),
                )
                .into()
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            EnumAudioEndpoints: EnumAudioEndpoints::<Identity, OFFSET>,
            GetDefaultAudioEndpoint: GetDefaultAudioEndpoint::<Identity, OFFSET>,
            GetDevice: GetDevice::<Identity, OFFSET>,
            RegisterEndpointNotificationCallback: RegisterEndpointNotificationCallback::<
                Identity,
                OFFSET,
            >,
            UnregisterEndpointNotificationCallback: UnregisterEndpointNotificationCallback::<
                Identity,
                OFFSET,
            >,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IMMDeviceEnumerator as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IMMDeviceEnumerator {}
windows_core::imp::define_interface!(
    IMMNotificationClient,
    IMMNotificationClient_Vtbl,
    0x7991eec9_7e89_4d85_8390_6c703cec60c0
);
windows_core::imp::interface_hierarchy!(IMMNotificationClient, windows_core::IUnknown);
#[repr(C)]
pub struct IMMNotificationClient_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    OnDeviceStateChanged: usize,
    OnDeviceAdded: usize,
    OnDeviceRemoved: usize,
    OnDefaultDeviceChanged: usize,
    OnPropertyValueChanged: usize,
}
impl windows_core::RuntimeName for IMMNotificationClient {}
windows_core::imp::define_interface!(
    IPropertyStore,
    IPropertyStore_Vtbl,
    0x886d8eeb_8cf2_4446_8d02_cdba1dbdcf99
);
windows_core::imp::interface_hierarchy!(IPropertyStore, windows_core::IUnknown);
impl IPropertyStore {
    pub(crate) unsafe fn GetCount(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetCount)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn GetAt(&self, iprop: u32, pkey: *mut PROPERTYKEY) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetAt)(
                windows_core::Interface::as_raw(self),
                iprop,
                pkey as _,
            )
        }
    }
    pub(crate) unsafe fn GetValue(
        &self,
        key: *const PROPERTYKEY,
    ) -> windows_core::Result<PROPVARIANT> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetValue)(
                windows_core::Interface::as_raw(self),
                key,
                &mut result__,
            )
            .map(|| core::mem::transmute(result__))
        }
    }
    pub(crate) unsafe fn SetValue(
        &self,
        key: *const PROPERTYKEY,
        propvar: *const PROPVARIANT,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetValue)(
                windows_core::Interface::as_raw(self),
                key,
                propvar,
            )
        }
    }
    pub(crate) unsafe fn Commit(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Commit)(windows_core::Interface::as_raw(self))
        }
    }
}
#[repr(C)]
pub struct IPropertyStore_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetAt: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut PROPERTYKEY,
    ) -> windows_core::HRESULT,
    pub GetValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const PROPERTYKEY,
        *mut PROPVARIANT,
    ) -> windows_core::HRESULT,
    pub SetValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const PROPERTYKEY,
        *const PROPVARIANT,
    ) -> windows_core::HRESULT,
    pub Commit: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
pub trait IPropertyStore_Impl: windows_core::IUnknownImpl {
    fn GetCount(&self) -> windows_core::Result<u32>;
    fn GetAt(&self, iprop: u32, pkey: *mut PROPERTYKEY) -> windows_core::Result<()>;
    fn GetValue(&self, key: *const PROPERTYKEY) -> windows_core::Result<PROPVARIANT>;
    fn SetValue(
        &self,
        key: *const PROPERTYKEY,
        propvar: *const PROPVARIANT,
    ) -> windows_core::Result<()>;
    fn Commit(&self) -> windows_core::Result<()>;
}
impl IPropertyStore_Vtbl {
    pub const fn new<Identity: IPropertyStore_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn GetCount<Identity: IPropertyStore_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            cprops: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IPropertyStore_Impl::GetCount(this) {
                    Ok(ok__) => {
                        cprops.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetAt<Identity: IPropertyStore_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            iprop: u32,
            pkey: *mut PROPERTYKEY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IPropertyStore_Impl::GetAt(
                    this,
                    core::mem::transmute_copy(&iprop),
                    core::mem::transmute_copy(&pkey),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetValue<Identity: IPropertyStore_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            key: *const PROPERTYKEY,
            pv: *mut PROPVARIANT,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IPropertyStore_Impl::GetValue(this, core::mem::transmute_copy(&key)) {
                    Ok(ok__) => {
                        pv.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn SetValue<Identity: IPropertyStore_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            key: *const PROPERTYKEY,
            propvar: *const PROPVARIANT,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IPropertyStore_Impl::SetValue(
                    this,
                    core::mem::transmute_copy(&key),
                    core::mem::transmute_copy(&propvar),
                )
                .into()
            }
        }
        unsafe extern "system" fn Commit<Identity: IPropertyStore_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IPropertyStore_Impl::Commit(this).into()
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            GetCount: GetCount::<Identity, OFFSET>,
            GetAt: GetAt::<Identity, OFFSET>,
            GetValue: GetValue::<Identity, OFFSET>,
            SetValue: SetValue::<Identity, OFFSET>,
            Commit: Commit::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IPropertyStore as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IPropertyStore {}
windows_core::imp::define_interface!(
    ISequentialStream,
    ISequentialStream_Vtbl,
    0x0c733a30_2a1c_11ce_ade5_00aa0044773d
);
windows_core::imp::interface_hierarchy!(ISequentialStream, windows_core::IUnknown);
#[repr(C)]
pub struct ISequentialStream_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    Read: usize,
    Write: usize,
}
impl windows_core::RuntimeName for ISequentialStream {}
windows_core::imp::define_interface!(
    IStorage,
    IStorage_Vtbl,
    0x0000000b_0000_0000_c000_000000000046
);
windows_core::imp::interface_hierarchy!(IStorage, windows_core::IUnknown);
#[repr(C)]
pub struct IStorage_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    CreateStream: usize,
    OpenStream: usize,
    CreateStorage: usize,
    OpenStorage: usize,
    CopyTo: usize,
    MoveElementTo: usize,
    Commit: usize,
    Revert: usize,
    EnumElements: usize,
    DestroyElement: usize,
    RenameElement: usize,
    SetElementTimes: usize,
    SetClass: usize,
    SetStateBits: usize,
    Stat: usize,
}
impl windows_core::RuntimeName for IStorage {}
windows_core::imp::define_interface!(
    IStream,
    IStream_Vtbl,
    0x0000000c_0000_0000_c000_000000000046
);
impl core::ops::Deref for IStream {
    type Target = ISequentialStream;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IStream, windows_core::IUnknown, ISequentialStream);
#[repr(C)]
pub struct IStream_Vtbl {
    pub base__: ISequentialStream_Vtbl,
    Seek: usize,
    SetSize: usize,
    CopyTo: usize,
    Commit: usize,
    Revert: usize,
    LockRegion: usize,
    UnlockRegion: usize,
    Stat: usize,
    Clone: usize,
}
impl windows_core::RuntimeName for IStream {}
pub type LPSAFEARRAY = *mut SAFEARRAY;
pub type LPVERSIONEDSTREAM = *mut VERSIONEDSTREAM;
pub const MMDeviceEnumerator: windows_core::GUID =
    windows_core::GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
pub const PKEY_Device_FriendlyName: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows_core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PROPERTYKEY {
    pub fmtid: windows_core::GUID,
    pub pid: u32,
}
#[repr(C)]
pub struct PROPVARIANT {
    pub Anonymous: PROPVARIANT_0,
}
impl Clone for PROPVARIANT {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
}
impl Default for PROPVARIANT {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
pub union PROPVARIANT_0 {
    pub Anonymous: core::mem::ManuallyDrop<PROPVARIANT_0_0>,
    pub decVal: DECIMAL,
}
impl Clone for PROPVARIANT_0 {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
}
impl Default for PROPVARIANT_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
pub struct PROPVARIANT_0_0 {
    pub vt: VARTYPE,
    pub wReserved1: PROPVAR_PAD1,
    pub wReserved2: PROPVAR_PAD2,
    pub wReserved3: PROPVAR_PAD3,
    pub Anonymous: PROPVARIANT_0_0_0,
}
impl Clone for PROPVARIANT_0_0 {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
}
impl Default for PROPVARIANT_0_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
pub union PROPVARIANT_0_0_0 {
    pub cVal: i8,
    pub bVal: u8,
    pub iVal: i16,
    pub uiVal: u16,
    pub lVal: i32,
    pub ulVal: u32,
    pub intVal: i32,
    pub uintVal: u32,
    pub hVal: i64,
    pub uhVal: u64,
    pub fltVal: f32,
    pub dblVal: f64,
    pub boolVal: VARIANT_BOOL,
    pub __OBSOLETE__VARIANT_BOOL: VARIANT_BOOL,
    pub scode: SCODE,
    pub cyVal: CY,
    pub date: f64,
    pub filetime: FILETIME,
    pub puuid: *mut windows_core::GUID,
    pub pclipdata: *mut CLIPDATA,
    pub bstrVal: core::mem::ManuallyDrop<windows_core::BSTR>,
    pub bstrblobVal: BSTRBLOB,
    pub blob: BLOB,
    pub pszVal: windows_core::PSTR,
    pub pwszVal: windows_core::PWSTR,
    pub punkVal: core::mem::ManuallyDrop<Option<windows_core::IUnknown>>,
    pub pdispVal: core::mem::ManuallyDrop<Option<IDispatch>>,
    pub pStream: core::mem::ManuallyDrop<Option<IStream>>,
    pub pStorage: core::mem::ManuallyDrop<Option<IStorage>>,
    pub pVersionedStream: LPVERSIONEDSTREAM,
    pub parray: LPSAFEARRAY,
    pub cac: CAC,
    pub caub: CAUB,
    pub cai: CAI,
    pub caui: CAUI,
    pub cal: CAL,
    pub caul: CAUL,
    pub cah: CAH,
    pub cauh: CAUH,
    pub caflt: CAFLT,
    pub cadbl: CADBL,
    pub cabool: CABOOL,
    pub cascode: CASCODE,
    pub cacy: CACY,
    pub cadate: CADATE,
    pub cafiletime: CAFILETIME,
    pub cauuid: CACLSID,
    pub caclipdata: CACLIPDATA,
    pub cabstr: CABSTR,
    pub cabstrblob: CABSTRBLOB,
    pub calpstr: CALPSTR,
    pub calpwstr: CALPWSTR,
    pub capropvar: CAPROPVARIANT,
    pub pcVal: *mut i8,
    pub pbVal: *mut u8,
    pub piVal: *mut i16,
    pub puiVal: *mut u16,
    pub plVal: *mut i32,
    pub pulVal: *mut u32,
    pub pintVal: *mut i32,
    pub puintVal: *mut u32,
    pub pfltVal: *mut f32,
    pub pdblVal: *mut f64,
    pub pboolVal: *mut VARIANT_BOOL,
    pub pdecVal: *mut DECIMAL,
    pub pscode: *mut SCODE,
    pub pcyVal: *mut CY,
    pub pdate: *mut f64,
    pub pbstrVal: *mut windows_core::BSTR,
    pub ppunkVal: *mut Option<windows_core::IUnknown>,
    pub ppdispVal: *mut Option<IDispatch>,
    pub pparray: *mut LPSAFEARRAY,
    pub pvarVal: *mut PROPVARIANT,
}
impl Clone for PROPVARIANT_0_0_0 {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
}
impl Default for PROPVARIANT_0_0_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PROPVAR_PAD1(pub u16);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PROPVAR_PAD2(pub u16);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PROPVAR_PAD3(pub u16);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct REFERENCE_TIME(pub i64);
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SAFEARRAY {
    pub cDims: u16,
    pub fFeatures: u16,
    pub cbElements: u32,
    pub cLocks: u32,
    pub pvData: *mut core::ffi::c_void,
    pub rgsabound: [SAFEARRAYBOUND; 1],
}
impl Default for SAFEARRAY {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SAFEARRAYBOUND {
    pub cElements: u32,
    pub lLbound: i32,
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SCODE(pub i32);
pub type VARENUM = i32;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct VARIANT_BOOL(pub i16);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct VARTYPE(pub u16);
#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VERSIONEDSTREAM {
    pub guidVersion: windows_core::GUID,
    pub pStream: core::mem::ManuallyDrop<Option<IStream>>,
}
pub const VT_LPWSTR: VARENUM = 31;
#[repr(C, packed(1))]
#[derive(Clone, Copy, Default)]
pub struct WAVEFORMATEX {
    pub wFormatTag: u16,
    pub nChannels: u16,
    pub nSamplesPerSec: u32,
    pub nAvgBytesPerSec: u32,
    pub nBlockAlign: u16,
    pub wBitsPerSample: u16,
    pub cbSize: u16,
}
#[repr(C, packed(1))]
#[derive(Clone, Copy)]
pub struct WAVEFORMATEXTENSIBLE {
    pub Format: WAVEFORMATEX,
    pub Samples: WAVEFORMATEXTENSIBLE_0,
    pub dwChannelMask: u32,
    pub SubFormat: windows_core::GUID,
}
impl Default for WAVEFORMATEXTENSIBLE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C, packed(1))]
#[derive(Clone, Copy)]
pub union WAVEFORMATEXTENSIBLE_0 {
    pub wValidBitsPerSample: u16,
    pub wSamplesPerBlock: u16,
    pub wReserved: u16,
}
impl Default for WAVEFORMATEXTENSIBLE_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub const WAVE_FORMAT_EXTENSIBLE: i32 = 65534;
pub const WAVE_FORMAT_IEEE_FLOAT: i32 = 3;
pub const WAVE_FORMAT_PCM: i32 = 1;
pub type _AUDCLNT_BUFFERFLAGS = i32;
pub const eAll: EDataFlow = 2;
pub const eCapture: EDataFlow = 1;
pub const eCommunications: ERole = 2;
pub const eConsole: ERole = 0;
pub const eMultimedia: ERole = 1;
pub const eRender: EDataFlow = 0;
