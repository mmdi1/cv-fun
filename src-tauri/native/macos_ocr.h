#pragma once

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Recognize text in an image file (local Apple Vision).
 * Returns a malloc'd UTF-8 C string on success (caller must nfun_ocr_free).
 * On failure returns NULL and, if out_err is non-NULL, sets *out_err to a
 * malloc'd error message (caller must nfun_ocr_free).
 */
char *nfun_ocr_image_at_path(const char *path_utf8, char **out_err);

void nfun_ocr_free(char *s);

#ifdef __cplusplus
}
#endif
