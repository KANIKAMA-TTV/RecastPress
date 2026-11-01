/* RecastPress v0.5 P0: bounded, side-effect-free managed path access query.
 * This header is installed only by the opt-in source patch. It must never use
 * PHP streams/stat or change an open result: the Rust VFS remains authoritative.
 */
#ifndef RECASTPRESS_FS_ACCESS_H
#define RECASTPRESS_FS_ACCESS_H

#if defined(__wasi__)
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#if F_OK != 0 || X_OK != 1 || W_OK != 2 || R_OK != 4
# error "The managed access wire requires POSIX F/X/W/R mode bits 0/1/2/4"
#endif

#define RP_FS_ACCESS_PATH "/rp-p0-fs/access"
#define RP_FS_ACCESS_MAX_PATH 4096
#define RP_FS_ACCESS_ALLOW 0
#define RP_FS_ACCESS_DENY 1
#define RP_FS_ACCESS_MISSING 2
#define RP_FS_ACCESS_UNMANAGED 3
#define RP_FS_ACCESS_INVALID 4
#define RP_FS_ACCESS_IO 5
#define RP_FS_ACCESS_LOCAL_FAILURE (-1)

/* Strict UTF-8 scalar values: no overlong, surrogate or out-of-range sequence. */
static inline int rp_fs_access_utf8(const unsigned char *path, size_t length)
{
    size_t i = 0;
    while (i < length) {
        unsigned char a = path[i++];
        if (a < 0x80) {
            if (a == 0) { return 0; }
            continue;
        }
        if (a >= 0xc2 && a <= 0xdf) {
            if (length - i < 1 || path[i] < 0x80 || path[i] > 0xbf) { return 0; }
            i++;
        } else if (a >= 0xe0 && a <= 0xef) {
            unsigned char b;
            if (length - i < 2) { return 0; }
            b = path[i];
            if (b < 0x80 || b > 0xbf || (a == 0xe0 && b < 0xa0) || (a == 0xed && b >= 0xa0)
                    || path[i + 1] < 0x80 || path[i + 1] > 0xbf) { return 0; }
            i += 2;
        } else if (a >= 0xf0 && a <= 0xf4) {
            unsigned char b;
            if (length - i < 3) { return 0; }
            b = path[i];
            if (b < 0x80 || b > 0xbf || (a == 0xf0 && b < 0x90) || (a == 0xf4 && b > 0x8f)
                    || path[i + 1] < 0x80 || path[i + 1] > 0xbf
                    || path[i + 2] < 0x80 || path[i + 2] > 0xbf) { return 0; }
            i += 3;
        } else {
            return 0;
        }
    }
    return 1;
}

static inline int rp_fs_access_absolute(const char *path,
        char absolute[RP_FS_ACCESS_MAX_PATH + 1], size_t *length)
{
    size_t path_length = 0;
    size_t cwd_length = 0;
    size_t separator;
    if (path == NULL || length == NULL || path[0] == '\0') { errno = EINVAL; return -1; }
    while (path_length <= RP_FS_ACCESS_MAX_PATH && path[path_length] != '\0') { path_length++; }
    if (path_length > RP_FS_ACCESS_MAX_PATH) { errno = ENAMETOOLONG; return -1; }
    if (path[0] == '/') {
        memcpy(absolute, path, path_length + 1);
        *length = path_length;
    } else {
        /* Respect PHP's virtual cwd without entering its stream/stat layer. */
#ifdef VCWD_GETCWD
        if (VCWD_GETCWD(absolute, RP_FS_ACCESS_MAX_PATH + 1) == NULL) {
#else
        if (getcwd(absolute, RP_FS_ACCESS_MAX_PATH + 1) == NULL) {
#endif
            if (errno == ERANGE) { errno = ENAMETOOLONG; }
            return -1;
        }
        while (cwd_length <= RP_FS_ACCESS_MAX_PATH && absolute[cwd_length] != '\0') { cwd_length++; }
        if (cwd_length == 0 || cwd_length > RP_FS_ACCESS_MAX_PATH || absolute[0] != '/') {
            errno = cwd_length > RP_FS_ACCESS_MAX_PATH ? ENAMETOOLONG : EINVAL;
            return -1;
        }
        separator = absolute[cwd_length - 1] == '/' ? 0 : 1;
        if (path_length > RP_FS_ACCESS_MAX_PATH - cwd_length
                || separator > RP_FS_ACCESS_MAX_PATH - cwd_length - path_length) {
            errno = ENAMETOOLONG;
            return -1;
        }
        if (separator != 0) { absolute[cwd_length++] = '/'; }
        memcpy(absolute + cwd_length, path, path_length + 1);
        *length = cwd_length + path_length;
    }
    if (!rp_fs_access_utf8((const unsigned char *)absolute, *length)) { errno = EINVAL; return -1; }
    return 0;
}

/* One libc FILE open, one request, one exact response and one close. No cache.
 * LOCAL_FAILURE retains the local path/mode error; transport failure is EIO.
 * Only an explicit complete UNMANAGED response permits the caller's fallback.
 */
static inline int rp_fs_access_query(const char *path, int mode)
{
    char absolute[RP_FS_ACCESS_MAX_PATH + 1];
    unsigned char header[7] = {'R', 'P', 'A', '1', 0, 0, 0};
    unsigned char response[5];
    size_t length = 0;
    FILE *channel;
    int incoming_errno = errno;
    int complete = 0;
    int status = RP_FS_ACCESS_IO;

    if (mode < 0 || mode > 7) { errno = EINVAL; return RP_FS_ACCESS_LOCAL_FAILURE; }
    if (rp_fs_access_absolute(path, absolute, &length) != 0) { return RP_FS_ACCESS_LOCAL_FAILURE; }
    header[4] = (unsigned char)mode;
    header[5] = (unsigned char)(length & 0xff);
    header[6] = (unsigned char)((length >> 8) & 0xff);
    channel = fopen(RP_FS_ACCESS_PATH, "r+b");
    if (channel == NULL) { errno = EIO; return RP_FS_ACCESS_IO; }
    if (setvbuf(channel, NULL, _IONBF, 0) != 0) { goto close_channel; }
    if (fwrite(header, 1, sizeof(header), channel) != sizeof(header) || ferror(channel)) { goto close_channel; }
    if (fwrite(absolute, 1, length, channel) != length || ferror(channel)) { goto close_channel; }
    if (fflush(channel) != 0 || ferror(channel)) { goto close_channel; }
    if (fseek(channel, 0, SEEK_SET) != 0 || ferror(channel)) { goto close_channel; }
    if (fread(response, 1, sizeof(response), channel) != sizeof(response) || ferror(channel)) { goto close_channel; }
    if (fgetc(channel) != EOF || ferror(channel)) { goto close_channel; }
    if (memcmp(response, "RPR1", 4) != 0 || response[4] > RP_FS_ACCESS_IO) { goto close_channel; }
    status = response[4];
    complete = 1;
close_channel:
    if (fclose(channel) != 0) { complete = 0; }
    if (!complete) { errno = EIO; return RP_FS_ACCESS_IO; }
    switch (status) {
        case RP_FS_ACCESS_ALLOW:
        case RP_FS_ACCESS_UNMANAGED: errno = incoming_errno; break;
        case RP_FS_ACCESS_DENY: errno = EACCES; break;
        case RP_FS_ACCESS_MISSING: errno = ENOENT; break;
        case RP_FS_ACCESS_INVALID: errno = EINVAL; break;
        default: errno = EIO; break;
    }
    return status;
}

/* Only change a failed open's EPERM after the SAME resolved path and POSIX
 * access mode receive an explicit Rust EACCES denial. No open is retried.
 * Unknown/missing/allow, local errors and broken transport preserve EPERM.
 */
static inline void rp_fs_access_correct_open_errno(const char *path, int open_flags)
{
    int incoming_errno = errno;
    int mode;
    int status;
    if (incoming_errno != EPERM) { return; }
    switch (open_flags & O_ACCMODE) {
        case O_RDONLY: mode = R_OK; break;
        case O_WRONLY: mode = W_OK; break;
        case O_RDWR: mode = R_OK | W_OK; break;
        default: errno = incoming_errno; return;
    }
    status = rp_fs_access_query(path, mode);
    errno = status == RP_FS_ACCESS_DENY ? EACCES : incoming_errno;
}
#endif /* __wasi__ */
#endif /* RECASTPRESS_FS_ACCESS_H */
