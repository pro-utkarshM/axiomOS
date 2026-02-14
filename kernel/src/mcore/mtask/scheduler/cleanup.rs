use alloc::boxed::Box;
use core::pin::Pin;
use core::ptr;

use conquer_once::spin::OnceCell;

use crate::mcore::mtask::process::Process;
use crate::mcore::mtask::scheduler::global::GlobalTaskQueue;
use crate::mcore::mtask::task::{Task, TaskQueue};

static CLEANUP_QUEUE: OnceCell<TaskQueue> = OnceCell::uninit();

fn cleanup_queue() -> &'static TaskQueue {
    CLEANUP_QUEUE.get().expect("TaskCleanup not initialized")
}

pub struct TaskCleanup;

impl TaskCleanup {
    pub fn init() {
        // log::info!("TaskCleanup: initializing...");
        CLEANUP_QUEUE.init_once(TaskQueue::new);
        let task = Task::create_new(Process::root(), Self::run, ptr::null_mut())
            .expect("should be able to create task cleanup");
        // log::info!("TaskCleanup: task created, enqueuing...");
        GlobalTaskQueue::enqueue(Box::pin(task));
        // log::info!("TaskCleanup: initialized");
    }

    pub fn enqueue(task: Pin<Box<Task>>) {
        // log::trace!("TaskCleanup: received zombie task {}", task.id());
        cleanup_queue().enqueue(task);
    }

    extern "C" fn run(_arg: *mut core::ffi::c_void) {
        // log::info!("TaskCleanup: running");
        loop {
            while let Some(task) = cleanup_queue().dequeue() {
                // log::trace!("TaskCleanup: cleaning up task {}", task.id());
                drop(task);
            }

            #[cfg(target_arch = "x86_64")]
            x86_64::instructions::hlt();
            #[cfg(target_arch = "aarch64")]
            unsafe {
                core::arch::asm!("wfi");
            }
        }
    }
}
