#import <Foundation/Foundation.h>
#import "Local.h" // fallback

#define kTimeout 30 // seconds

// This line comment should be removed
/* This block comment should be removed */
/// This doc comment should not be removed

NSString *url = @"http://example.com//path";
NSString *msg = @"Hello // world";
char *c = "http://example.com//path";
// TODO: check return value
NSLog(@"%@ msg", msg);
NSLog(@"%@", url); // trailing, should be removed

@implementation Example
- (void)hello {
    // Block comment inside method
    NSLog(@"Hello, Objective-C!");
}
@end
