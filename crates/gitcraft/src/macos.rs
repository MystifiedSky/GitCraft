//! macOS: `gitcraft://` links (the Get URL Apple event) and the Finder's "Open in GitCraft"
//! service. Both hand what they were given to the window like a second launch would.

use crate::instance;
use gitcraft_core::session::{LaunchIntent, clone_launch};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSPasteboard};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSArray, NSObject, NSString, NSURL};
use std::cell::OnceCell;
use std::path::{Path, PathBuf};

/// Finder uses a system-only PATH. Preserve the user's tool order and add missing
/// package managers before the OS shims.
///
/// # Safety
/// Call before starting any threads or initializing AppKit.
pub(super) unsafe fn add_tool_paths() {
    let existing = std::env::var_os("PATH").filter(|path| !path.is_empty()).unwrap_or_else(|| "/usr/bin:/bin:/usr/sbin:/sbin".into());
    let mut folders: Vec<_> = std::env::split_paths(&existing).filter(|path| path.is_absolute()).collect();
    let package_managers = if cfg!(target_arch = "aarch64") { ["/opt/homebrew/bin", "/usr/local/bin"] } else { ["/usr/local/bin", "/opt/homebrew/bin"] };
    let missing = package_managers.into_iter().map(PathBuf::from).filter(|path| path.is_dir() && !folders.contains(path)).collect::<Vec<_>>();
    let system = folders.iter().position(|path| path == Path::new("/usr/bin")).unwrap_or(folders.len());
    folders.splice(system..system, missing);
    if let Ok(path) = std::env::join_paths(folders) {
        // SAFETY: The caller ensures that no other threads can read the environment.
        unsafe { std::env::set_var("PATH", path) };
    }
}

/// 'GURL': the event class and id of "open this URL".
const GET_URL: u32 = u32::from_be_bytes(*b"GURL");
/// '----': the event's direct parameter, the URL itself.
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct GitCraftLinks;

    impl GitCraftLinks {
        #[unsafe(method(handleGetURL:withReplyEvent:))]
        fn handle_get_url(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            // SAFETY: AppKit hands a valid Apple event to this handler on the main thread.
            let param: Option<Retained<NSAppleEventDescriptor>> = unsafe { msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT] };
            let Some(text) = param.and_then(|p| p.stringValue()) else { return };
            if let Some(intent) = clone_launch(&text.to_string()) {
                instance::deliver(vec![intent]);
            }
        }

        #[unsafe(method(openInGitCraft:userData:error:))]
        fn open_in_gitcraft(&self, pasteboard: &NSPasteboard, _user_data: Option<&NSString>, error: *mut *mut NSString) {
            let classes = NSArray::from_slice(&[NSURL::class()]);
            // SAFETY: AppKit delivers a valid pasteboard to this service callback on the main thread.
            let files = unsafe { pasteboard.readObjectsForClasses_options(&classes, None) };
            let paths: Vec<_> = files
                .iter()
                .flat_map(|files| files.iter())
                .filter_map(|object| object.downcast::<NSURL>().ok())
                .filter(|url| url.isFileURL())
                .filter_map(|url| url.path().map(|path| PathBuf::from(path.to_string())))
                .filter_map(|path| {
                    let path = if path.is_dir() { path } else { path.parent()?.to_owned() };
                    Some(LaunchIntent::Repository { path: path.to_string_lossy().into_owned() })
                })
                .collect();
            if paths.is_empty() {
                // SAFETY: AppKit passes either null or a writable out-pointer for the error string.
                if !error.is_null() {
                    unsafe { *error = std::ptr::null_mut() };
                }
                return;
            }
            instance::deliver(paths);
        }
    }
);

thread_local! {
    static HANDLER: OnceCell<Retained<GitCraftLinks>> = const { OnceCell::new() };
}

fn handler(main: MainThreadMarker) -> Retained<GitCraftLinks> {
    HANDLER.with(|cell| {
        // SAFETY: `init` is NSObject's designated initializer for this freshly allocated instance.
        cell.get_or_init(|| unsafe { msg_send![GitCraftLinks::alloc(main), init] }).clone()
    })
}

/// Take `gitcraft://` links. Called before the window opens, so the link that started GitCraft
/// is not missed.
pub fn handle_links() {
    let Some(main) = MainThreadMarker::new() else { return };
    let handler = handler(main);
    let manager = NSAppleEventManager::sharedAppleEventManager();
    let target: &AnyObject = &handler;
    // SAFETY: main-thread call; the handler implements the selector and lives for the process.
    unsafe {
        let _: () = msg_send![&manager, setEventHandler: target, andSelector: sel!(handleGetURL:withReplyEvent:), forEventClass: GET_URL, andEventID: GET_URL];
    }
}

/// Offer the Finder service. Called once the application object exists.
pub fn register_services() {
    let Some(main) = MainThreadMarker::new() else { return };
    let handler = handler(main);
    // SAFETY: main-thread AppKit call; the provider is retained for the life of the process.
    unsafe { NSApplication::sharedApplication(main).setServicesProvider(Some(&*handler)) };
}
