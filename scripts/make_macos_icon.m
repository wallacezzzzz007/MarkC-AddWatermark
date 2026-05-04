#import <AppKit/AppKit.h>

int main(int argc, const char * argv[]) {
    @autoreleasepool {
        if (argc != 3) {
            fprintf(stderr, "Usage: make_macos_icon <input.png> <output.png>\n");
            return 1;
        }

        NSString *inputPath = [NSString stringWithUTF8String:argv[1]];
        NSString *outputPath = [NSString stringWithUTF8String:argv[2]];
        NSImage *source = [[NSImage alloc] initWithContentsOfFile:inputPath];
        if (!source) {
            fprintf(stderr, "Could not read input image\n");
            return 1;
        }

        CGFloat size = 1024.0;
        NSImage *image = [[NSImage alloc] initWithSize:NSMakeSize(size, size)];
        [image lockFocus];

        [[NSColor clearColor] setFill];
        NSRectFill(NSMakeRect(0, 0, size, size));

        NSRect tileRect = NSMakeRect(64, 64, 896, 896);
        NSBezierPath *tilePath = [NSBezierPath bezierPathWithRoundedRect:tileRect xRadius:204 yRadius:204];

        [NSGraphicsContext saveGraphicsState];
        NSShadow *shadow = [[NSShadow alloc] init];
        [shadow setShadowColor:[[NSColor blackColor] colorWithAlphaComponent:0.20]];
        [shadow setShadowBlurRadius:34];
        [shadow setShadowOffset:NSMakeSize(0, -10)];
        [shadow set];
        [[NSColor whiteColor] setFill];
        [tilePath fill];
        [NSGraphicsContext restoreGraphicsState];

        [[[NSColor blackColor] colorWithAlphaComponent:0.06] setStroke];
        [tilePath setLineWidth:2];
        [tilePath stroke];

        NSRect logoRect = NSMakeRect(132, 132, 760, 760);
        [source drawInRect:logoRect fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:1.0];

        [image unlockFocus];

        CGImageRef cgImage = [image CGImageForProposedRect:NULL context:nil hints:nil];
        if (!cgImage) {
            fprintf(stderr, "Could not create CGImage\n");
            return 1;
        }

        NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:cgImage];
        NSData *png = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
        if (!png) {
            fprintf(stderr, "Could not encode PNG data\n");
            return 1;
        }

        NSURL *outputURL = [NSURL fileURLWithPath:outputPath];
        NSError *writeError = nil;
        if (![png writeToURL:outputURL options:NSDataWritingAtomic error:&writeError]) {
            fprintf(stderr, "Could not write output image: %s\n", [[writeError localizedDescription] UTF8String]);
            fprintf(stderr, "Could not write output image\n");
            return 1;
        }
    }
    return 0;
}
