#import "macos_ocr.h"
#import <AppKit/AppKit.h>
#import <Foundation/Foundation.h>
#import <Vision/Vision.h>

#include <stdlib.h>
#include <string.h>

// No @available() checks here: they emit ___isPlatformVersionAtLeast which
// rustc's linker does not pull in from clang_rt. Deployment target is set in
// build.rs (macOS 11+). Vision text OCR requires 10.15+; we document that.

void nfun_ocr_free(char *s) {
  if (s) {
    free(s);
  }
}

static char *dup_nsstring(NSString *s) {
  if (!s) {
    return NULL;
  }
  const char *utf8 = [s UTF8String];
  if (!utf8) {
    return NULL;
  }
  return strdup(utf8);
}

char *nfun_ocr_image_at_path(const char *path_utf8, char **out_err) {
  @autoreleasepool {
    if (out_err) {
      *out_err = NULL;
    }
    if (!path_utf8 || path_utf8[0] == '\0') {
      if (out_err) {
        *out_err = strdup("图片路径为空");
      }
      return NULL;
    }

    NSString *path = [NSString stringWithUTF8String:path_utf8];
    if (!path.length) {
      if (out_err) {
        *out_err = strdup("无效图片路径");
      }
      return NULL;
    }

    NSURL *url = [NSURL fileURLWithPath:path isDirectory:NO];
    if (!url) {
      if (out_err) {
        *out_err = strdup("无法解析图片路径");
      }
      return NULL;
    }

    if (![[NSFileManager defaultManager] fileExistsAtPath:path]) {
      if (out_err) {
        *out_err = strdup("图片文件不存在");
      }
      return NULL;
    }

    __block NSMutableString *collected = [NSMutableString string];
    __block NSError *handlerError = nil;

    VNRecognizeTextRequest *request = [[VNRecognizeTextRequest alloc]
        initWithCompletionHandler:^(VNRequest *req, NSError *error) {
          if (error) {
            handlerError = error;
            return;
          }
          NSArray *results = req.results;
          if (![results isKindOfClass:[NSArray class]]) {
            return;
          }
          // Sort top-to-bottom, then left-to-right for reading order
          NSArray *sorted =
              [results sortedArrayUsingComparator:^NSComparisonResult(id a, id b) {
                VNRecognizedTextObservation *oa = (VNRecognizedTextObservation *)a;
                VNRecognizedTextObservation *ob = (VNRecognizedTextObservation *)b;
                CGFloat ya = CGRectGetMidY(oa.boundingBox);
                CGFloat yb = CGRectGetMidY(ob.boundingBox);
                if (fabs(ya - yb) > 0.02) {
                  // Vision coords: origin bottom-left; higher y is higher on page
                  return ya < yb ? NSOrderedDescending : NSOrderedAscending;
                }
                CGFloat xa = CGRectGetMinX(oa.boundingBox);
                CGFloat xb = CGRectGetMinX(ob.boundingBox);
                return xa < xb ? NSOrderedAscending
                               : (xa > xb ? NSOrderedDescending : NSOrderedSame);
              }];

          for (VNRecognizedTextObservation *obs in sorted) {
            if (![obs isKindOfClass:[VNRecognizedTextObservation class]]) {
              continue;
            }
            VNRecognizedText *top = [[obs topCandidates:1] firstObject];
            if (top.string.length == 0) {
              continue;
            }
            if (collected.length > 0) {
              [collected appendString:@"\n"];
            }
            [collected appendString:top.string];
          }
        }];

    request.recognitionLevel = VNRequestTextRecognitionLevelAccurate;
    request.usesLanguageCorrection = YES;
    // Prefer Chinese + English; no macOS 12-only language probe API.
    request.recognitionLanguages = @[ @"zh-Hans", @"zh-Hant", @"en-US" ];

    VNImageRequestHandler *handler =
        [[VNImageRequestHandler alloc] initWithURL:url options:@{}];
    NSError *perfError = nil;
    BOOL ok = [handler performRequests:@[ request ] error:&perfError];
    if (!ok || perfError || handlerError) {
      NSError *e = perfError ?: handlerError;
      NSString *msg = e.localizedDescription ?: @"Vision OCR 失败";
      if (out_err) {
        *out_err = dup_nsstring(msg);
      }
      return NULL;
    }

    if (collected.length == 0) {
      if (out_err) {
        *out_err = strdup("未识别到文字");
      }
      return NULL;
    }

    return dup_nsstring(collected);
  }
}
