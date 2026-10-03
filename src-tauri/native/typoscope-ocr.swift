import Foundation
import Vision
import CoreGraphics
import ImageIO

struct WordBox: Codable {
    let text: String
    let confidence: Float
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

func loadCGImage(path: String) -> CGImage? {
    let url = URL(fileURLWithPath: path) as CFURL
    guard let source = CGImageSourceCreateWithURL(url, nil) else { return nil }
    return CGImageSourceCreateImageAtIndex(source, 0, nil)
}

func hasVisiblePixels(_ image: CGImage) -> Bool {
    let size = 32
    var pixels = [UInt8](repeating: 0, count: size * size * 4)
    return pixels.withUnsafeMutableBytes { buffer in
        guard let context = CGContext(data: buffer.baseAddress, width: size, height: size,
                                      bitsPerComponent: 8, bytesPerRow: size * 4,
                                      space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        context.draw(image, in: CGRect(x: 0, y: 0, width: size, height: size))
        return stride(from: 3, to: buffer.count, by: 4).contains { buffer[$0] != 0 }
    }
}

func recognizeWords(in image: CGImage) throws -> [WordBox] {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false
    // Full-display sentence scans must still recognize normal-sized body text.
    request.minimumTextHeight = 0.004

    let handler = VNImageRequestHandler(cgImage: image, options: [:])
    try handler.perform([request])

    let imgW = Double(image.width)
    let imgH = Double(image.height)
    var boxes: [WordBox] = []

    guard let observations = request.results else { return boxes }

    for observation in observations {
        guard let candidate = observation.topCandidates(1).first else { continue }
        let text = candidate.string
        var didAddWord = false

        // Keep punctuation attached to tokens: .byWords discards sentence endings.
        // Substring indices belong to this same String, including Unicode text.
        for token in text.split(whereSeparator: { $0.isWhitespace }) {
            let substringRange = token.startIndex..<token.endIndex
            guard let box = try? candidate.boundingBox(for: substringRange)?.boundingBox else { continue }
            let pixel = normalizedToTopLeftPixels(box, width: imgW, height: imgH)
            boxes.append(WordBox(
                text: String(text[substringRange]),
                confidence: candidate.confidence,
                x: pixel.x,
                y: pixel.y,
                width: pixel.width,
                height: pixel.height
            ))
            didAddWord = true
        }

        if !didAddWord {
            let pixel = normalizedToTopLeftPixels(observation.boundingBox, width: imgW, height: imgH)
            boxes.append(WordBox(
                text: text,
                confidence: candidate.confidence,
                x: pixel.x,
                y: pixel.y,
                width: pixel.width,
                height: pixel.height
            ))
        }
    }

    return boxes
}

struct PixelRect {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

func normalizedToTopLeftPixels(_ box: CGRect, width: Double, height: Double) -> PixelRect {
    let x = box.origin.x * width
    let w = box.size.width * width
    let h = box.size.height * height
    let y = (1.0 - box.origin.y - box.size.height) * height
    return PixelRect(x: x, y: y, width: w, height: h)
}

guard CommandLine.arguments.count >= 2 else {
    fputs("usage: typoscope-ocr <image-path>\n", stderr)
    exit(2)
}

let path = CommandLine.arguments[1]
guard let image = loadCGImage(path: path) else {
    fputs("failed to load image at \(path)\n", stderr)
    exit(1)
}

guard hasVisiblePixels(image) else {
    fputs("capture is transparent — no readable windows; check Screen Recording permission for Typoscope / your terminal\n", stderr)
    exit(1)
}

do {
    let words = try recognizeWords(in: image)
    fputs(
        "OCR_META {\"imageWidth\":\(image.width),\"imageHeight\":\(image.height),\"wordCount\":\(words.count),\"path\":\"\(path)\"}\n",
        stderr
    )
    let data = try JSONEncoder().encode(words)
    if let json = String(data: data, encoding: .utf8) {
        print(json)
    }
} catch {
    fputs("ocr failed: \(error)\n", stderr)
    exit(1)
}
