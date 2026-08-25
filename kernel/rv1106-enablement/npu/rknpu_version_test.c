// SPDX-License-Identifier: GPL-2.0
/*
 * rknpu_version_test.c -- standalone hardware smoke test for the RKNPU
 * kernel driver port (see PORT-PLAN.md / PORT-PROGRESS.md in this
 * directory).
 *
 * Opens a DRM node (tries /dev/dri/card0, then card1, then renderD128),
 * issues DRM_IOCTL_RKNPU_ACTION with RKNPU_GET_DRV_VERSION and then
 * RKNPU_GET_HW_VERSION, and prints the decoded results. This exercises the
 * full ioctl-dispatch -> power-get/put -> clock/reset path with zero
 * dependency on a regcmd buffer or the (closed) RKNN runtime -- see
 * OPEN-NPU-PLAN.md §1.4 Tier A and PORT-PLAN.md §3 step 5.
 *
 * Dependency-free beyond the UAPI header: build with
 *   -I<kernel-tree>/include/uapi
 * so both <drm/drm.h> and <drm/rknpu_ioctl.h> (and the linux/types.h,
 * linux/ioctl.h they need) resolve entirely out of the kernel tree's own
 * include/uapi/ -- no libdrm, no target sysroot headers required.
 *
 * Cross-compile (see PORT-PROGRESS.md for the full recipe):
 *
 *   export PATH="/home/noah/projects/scada/flare-edge/sdk/tools/linux/toolchain/arm-rockchip830-linux-uclibcgnueabihf/bin:/usr/bin:/bin"
 *   arm-rockchip830-linux-uclibcgnueabihf-gcc \
 *     -I/home/noah/projects/scada/flare-edge/research/linux-6.18.46/include/uapi \
 *     -Wall -O2 -static -o rknpu_version_test rknpu_version_test.c
 *
 * (add -static if the target rootfs's libc version/ABI is in doubt; drop it
 * for a smaller dynamically-linked binary if the rootfs libc is known good.)
 *
 * Expected output on a working probe:
 *
 *   opened /dev/dri/card0 (fd=3)
 *   driver version: 0.9.2 (raw code 902)
 *   hw version: 0x.... (raw)
 *   PASS: /dev/dri/card0 answered both version-query ioctls -- probe,
 *   power-get/put, and clock/reset all exercised.
 *
 * "0.9.2" is DRIVER_MAJOR/MINOR/PATCHLEVEL from the vendor source
 * (rknpu_drv.h) unmodified by this port -- a match confirms the ioctl
 * round-trip reached real driver code, not a stub. The hw version is a raw
 * value read directly off the NPU core's VERSION/VERSION_NUM registers
 * (rknpu_job.c:rknpu_get_hw_version()) -- any non-zero, non-0xffffffff
 * value is a plausible "the register block is alive" signal; there is no
 * published decode table for it beyond that (PORT-PLAN.md §3 step 5).
 */

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#include <drm/rknpu_ioctl.h>

static const char *const candidates[] = {
	"/dev/dri/card0",
	"/dev/dri/card1",
	"/dev/dri/renderD128",
	NULL,
};

static int do_action(int fd, __u32 flags, __u32 *value)
{
	struct rknpu_action act;

	memset(&act, 0, sizeof(act));
	act.flags = flags;

	if (ioctl(fd, DRM_IOCTL_RKNPU_ACTION, &act) < 0)
		return -errno;

	*value = act.value;
	return 0;
}

int main(void)
{
	int fd = -1;
	const char *path = NULL;
	__u32 drv_version = 0;
	__u32 hw_version = 0;
	int ret;
	int i;

	/* There may be several DRM cards (the VOP display registers card0, the
	 * NPU registers its own). Try each candidate and keep the FIRST that
	 * actually answers the RKNPU version ioctl -- opening successfully is not
	 * enough (the display card opens fine but returns EINVAL). */
	for (i = 0; candidates[i] != NULL; i++) {
		int f = open(candidates[i], O_RDWR | O_CLOEXEC);
		if (f < 0) {
			fprintf(stderr, "open(%s): %s\n", candidates[i],
				strerror(errno));
			continue;
		}
		if (do_action(f, RKNPU_GET_DRV_VERSION, &drv_version) == 0) {
			fd = f;
			path = candidates[i];
			break;
		}
		fprintf(stderr, "%s: not an rknpu node (version ioctl: %s)\n",
			candidates[i], strerror(errno));
		close(f);
	}

	if (fd < 0) {
		fprintf(stderr,
			"FAIL: no DRM node answered the RKNPU version ioctl -- "
			"is CONFIG_ROCKCHIP_RKNPU probed? check "
			"`dmesg | grep -i rknpu` and `ls -la /dev/dri/`\n");
		return 1;
	}

	printf("opened %s (fd=%d)\n", path, fd);
	ret = 0;
	printf("driver version: %u.%u.%u (raw code %u)\n",
	       RKNPU_GET_DRV_VERSION_MAJOR(drv_version),
	       RKNPU_GET_DRV_VERSION_MINOR(drv_version),
	       RKNPU_GET_DRV_VERSION_PATCHLEVEL(drv_version), drv_version);

	ret = do_action(fd, RKNPU_GET_HW_VERSION, &hw_version);
	if (ret) {
		fprintf(stderr,
			"FAIL: DRM_IOCTL_RKNPU_ACTION(RKNPU_GET_HW_VERSION): %s\n",
			strerror(-ret));
		close(fd);
		return 1;
	}
	printf("hw version: 0x%08x (raw)\n", hw_version);

	printf("PASS: %s answered both version-query ioctls -- probe, "
	       "power-get/put, and clock/reset all exercised.\n", path);

	close(fd);
	return 0;
}
