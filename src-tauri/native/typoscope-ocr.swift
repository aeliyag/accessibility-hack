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

func recognizeWords(in image: CGImage) throws -> [WordBox] {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false

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

        text.enumerateSubstrings(in: text.startIndex..<text.endIndex, options: [.byWords]) { _, substringRange, _, _ in
            guard let box = try? candidate.boundingBox(for: substringRange)?.boundingBox else { return }
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
