import Foundation

/// Captures the main thread's program-counter chain for hang reporting.
///
/// Phase 29 sub-A. Used by `SentoriHangWatchdog` to fill an `anr` event's
/// `frames[].instructionAddress`. Server-side symbolicates against the
/// uploaded dSYM (Phase 22 sub-B). API surface and App Store review notes
/// are in `PRIVACY_AND_REVIEW.md`.
///
/// arm64-only. On non-arm64 (Intel Mac simulators) `captureMainThreadFrames`
/// returns an empty array; the watchdog falls back to its previous
/// `Thread.callStackSymbols` path.
@objc public final class SentoriThreadSampler: NSObject {

    /// Mach port for the main thread, captured once at SDK init time from
    /// main. 0 = uninstalled (or non-arm64 platform).
    private static var mainThreadHandle: thread_t = 0

    /// Captures the main thread's mach port. Must be called from the main
    /// thread, exactly once during SDK init. Idempotent — second + later
    /// calls are no-ops.
    @objc public static func installMainThreadHandle() {
        guard pthread_main_np() != 0 else { return }
        if mainThreadHandle != 0 { return }
        mainThreadHandle = pthread_mach_thread_np(pthread_self())
    }

    /// Walks the main thread's frame pointer chain and returns up to
    /// `maxFrames` PCs.
    ///
    /// - PC[0]   = current main-thread PC (from `thread_get_state`).
    /// - PC[i>0] = saved LR (return address) `i` frames up the stack.
    ///
    /// Returns an empty array if:
    ///   * the main handle isn't installed (caller forgot
    ///     `installMainThreadHandle` from main),
    ///   * the platform isn't arm64,
    ///   * `thread_get_state` or `vm_read_overwrite` fails,
    ///   * the caller is itself on main (we don't sample our own thread).
    ///
    /// Bridged to Objective-C as `[NSNumber]` of unsigned 64-bit PCs.
    @objc public static func captureMainThreadFrames(maxFrames: Int = 64) -> [NSNumber] {
        #if arch(arm64)
        guard mainThreadHandle != 0, maxFrames > 0 else { return [] }
        // Don't sample ourselves — we'd race with our own register state.
        if pthread_main_np() != 0 { return [] }

        var state = arm_thread_state64_t()
        // ARM_THREAD_STATE64_COUNT is a C macro that doesn't make the
        // Swift import; compute it the same way the macro does
        // (sizeof(arm_thread_state64_t) / sizeof(uint32_t)).
        var stateCount = mach_msg_type_number_t(
            MemoryLayout<arm_thread_state64_t>.size / MemoryLayout<UInt32>.size
        )

        let kr: kern_return_t = withUnsafeMutablePointer(to: &state) { sp in
            sp.withMemoryRebound(to: natural_t.self, capacity: Int(stateCount)) { ptr in
                thread_get_state(
                    mainThreadHandle,
                    ARM_THREAD_STATE64,
                    ptr,
                    &stateCount
                )
            }
        }
        guard kr == KERN_SUCCESS else { return [] }

        // The intrinsics __darwin_arm_thread_state64_get_{pc,fp} also
        // don't import cleanly into Swift. Reinterpret the struct as
        // a UInt64 array and pick out registers by ABI index, which is
        // identical between arm64 and arm64e (only the field names
        // differ; the raw byte layout is the same):
        //   __x[0..28] = indices 0..28
        //   __fp       = index 29
        //   __lr       = index 30
        //   __sp       = index 31
        //   __pc       = index 32
        //   __cpsr / __pad = index 33 (packed)
        // PAC bits, if any (arm64e), are stripped by stripPAC below.
        let regs = withUnsafeBytes(of: state) { raw -> (pc: UInt64, fp: UInt64) in
            let base = raw.baseAddress!.assumingMemoryBound(to: UInt64.self)
            return (pc: base[32], fp: base[29])
        }

        var frames: [UInt64] = []
        if regs.pc != 0 {
            frames.append(stripPAC(regs.pc))
        }

        var fp = regs.fp
        let task = mach_task_self_

        while frames.count < maxFrames && fp != 0 {
            // ARM64 frame layout (per AAPCS64): [saved fp][saved lr] at
            // offsets 0 and 8 from the current frame pointer.
            var nextFP: UInt64 = 0
            var savedLR: UInt64 = 0

            guard readWord(task: task, addr: fp, into: &nextFP) else { break }
            guard readWord(task: task, addr: fp &+ 8, into: &savedLR) else { break }

            // Sanity: FP must walk up the user stack (always increasing).
            // Bail on chain corruption; still return what we have.
            if nextFP <= fp { break }

            frames.append(stripPAC(savedLR))
            fp = nextFP
        }

        return frames.map { NSNumber(value: $0) }
        #else
        return []
        #endif
    }

    // MARK: - arm64 helpers

    #if arch(arm64)
    /// Strips the pointer authentication code (PAC) from a saved LR.
    /// On arm64 (non-arm64e) PAC bits aren't set; this is a no-op mask.
    /// On arm64e stack-saved LRs may carry a signing tag in the high
    /// bits; user-space PCs fit in the low 47 bits, so we mask above.
    private static func stripPAC(_ pc: UInt64) -> UInt64 {
        return pc & 0x0000_007F_FFFF_FFFF
    }

    /// `vm_read_overwrite` wrapper: read 8 bytes from `addr` in the given
    /// task into `dst`. Returns true iff the read succeeded with a full
    /// word transferred.
    private static func readWord(
        task: mach_port_t,
        addr: UInt64,
        into dst: inout UInt64
    ) -> Bool {
        var bytesRead: vm_size_t = 0
        return withUnsafeMutablePointer(to: &dst) { ptr -> Bool in
            let dstAddr = vm_address_t(UInt(bitPattern: UnsafeMutableRawPointer(ptr)))
            let kr = vm_read_overwrite(
                task,
                vm_address_t(addr),
                vm_size_t(MemoryLayout<UInt64>.size),
                dstAddr,
                &bytesRead
            )
            return kr == KERN_SUCCESS && bytesRead == MemoryLayout<UInt64>.size
        }
    }
    #endif
}
