"""Targeted Kani policy: one named harness at a time, under fixed limits.
定点 Kani 策略：每次只跑一个指定 harness，上限固定；未判定不得计为通过。
"""
import re

LOCK = "/tmp/jasper-kani.lock"
MAX_SECONDS = 600
MAX_VMEM_KB = 3500000
TIMEOUT_EXIT = 124
# Kani's own wording when CBMC dies under the limit, then allocator wording.
# Kani 在 CBMC 撞内存上限时的原文，其后是分配器报错原文。
MEMORY_PATTERNS = ["appears to have run out of memory", "std::bad_alloc", "Cannot allocate memory",
                   "out of memory", "memory allocation of"]


def valid_harness_name(name):
    return re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_][A-Za-z0-9_]*)*", name) is not None


def command(manifest, harness, seconds=MAX_SECONDS, vmem_kb=MAX_VMEM_KB):
    if not 1 <= seconds <= MAX_SECONDS or not 1 <= vmem_kb <= MAX_VMEM_KB:
        raise ValueError("Kani limits may be lowered but never raised")
    if not valid_harness_name(harness):
        raise ValueError("invalid harness name: " + harness)
    return ["flock", LOCK, "bash", "-c", 'ulimit -v "$1"; shift; timeout "$@"', "kani-limit",
            str(vmem_kb), str(seconds), "cargo", "kani", "--manifest-path", manifest, "--harness", harness]


def classify(returncode, output):
    """Return (status, reason); status is passed, undetermined or failed.
    返回（状态，原因）；只有恰好验证一个 harness 且成功才是 passed。
    """
    if any(pattern in output for pattern in MEMORY_PATTERNS):
        return "undetermined", "memory limit"
    if returncode == TIMEOUT_EXIT:
        return "undetermined", "time limit"
    if "Failed Checks:" in output:
        return "failed", "counterexample"
    if "VERIFICATION:- FAILED" in output:
        return "failed", "verification failed without a reported check"
    if returncode:
        return "failed", "command exit " + str(returncode)
    checked = len(re.findall(r"^Checking harness ", output, re.MULTILINE))
    successful = output.count("VERIFICATION:- SUCCESSFUL")
    if checked != 1 or successful != 1:
        return "failed", f"expected exactly one verified harness, saw {checked} checked and {successful} successful"
    return "passed", "exactly one harness verified"
