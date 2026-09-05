import AppKit
import CoreGraphics

let width = 1200
let height = 675
let canvasHeight = CGFloat(height)

func color(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> NSColor {
    NSColor(calibratedRed: r / 255, green: g / 255, blue: b / 255, alpha: a)
}

func drawText(_ text: String, x: CGFloat, y: CGFloat, size: CGFloat, weight: NSFont.Weight, color textColor: NSColor, name: String? = nil, tracking: CGFloat = 0) {
    let font = name.flatMap { NSFont(name: $0, size: size) } ?? NSFont.systemFont(ofSize: size, weight: weight)
    let paragraph = NSMutableParagraphStyle()
    paragraph.alignment = .left
    let attrs: [NSAttributedString.Key: Any] = [
        .font: font,
        .foregroundColor: textColor,
        .paragraphStyle: paragraph,
        .kern: tracking
    ]
    NSString(string: text).draw(at: CGPoint(x: x, y: canvasHeight - y), withAttributes: attrs)
}

func point(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
    CGPoint(x: x, y: canvasHeight - y)
}

func rect(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat) -> CGRect {
    CGRect(x: x, y: canvasHeight - y - h, width: w, height: h)
}

func roundedRect(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, _ r: CGFloat) -> NSBezierPath {
    NSBezierPath(roundedRect: rect(x, y, w, h), xRadius: r, yRadius: r)
}

func ellipse(_ cx: CGFloat, _ cy: CGFloat, _ r: CGFloat) -> CGRect {
    rect(cx - r, cy - r, r * 2, r * 2)
}

let image = NSImage(size: NSSize(width: width, height: height))
image.lockFocus()
guard let ctx = NSGraphicsContext.current?.cgContext else {
    fatalError("No graphics context")
}

let bg = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: [
    color(246, 241, 229).cgColor,
    color(232, 239, 224).cgColor,
    color(214, 229, 218).cgColor
] as CFArray, locations: [0, 0.58, 1])!
ctx.drawLinearGradient(bg, start: point(0, 0), end: point(CGFloat(width), CGFloat(height)), options: [])

ctx.setLineWidth(1)
ctx.setStrokeColor(color(28, 72, 59, 0.08).cgColor)
for x in stride(from: 0, through: width, by: 42) {
    ctx.move(to: point(CGFloat(x), 0))
    ctx.addLine(to: point(CGFloat(x), CGFloat(height)))
}
for y in stride(from: 0, through: height, by: 42) {
    ctx.move(to: point(0, CGFloat(y)))
    ctx.addLine(to: point(CGFloat(width), CGFloat(y)))
}
ctx.strokePath()

ctx.setFillColor(color(31, 91, 71, 0.12).cgColor)
let terrain = CGMutablePath()
terrain.move(to: point(0, 516))
terrain.addCurve(to: point(492, 520), control1: point(176, 460), control2: point(312, 578))
terrain.addCurve(to: point(992, 384), control1: point(674, 462), control2: point(780, 346))
terrain.addCurve(to: point(1200, 430), control1: point(1092, 402), control2: point(1162, 456))
terrain.addLine(to: point(1200, CGFloat(height)))
terrain.addLine(to: point(0, CGFloat(height)))
terrain.closeSubpath()
ctx.addPath(terrain)
ctx.fillPath()

ctx.setStrokeColor(color(31, 91, 71, 0.18).cgColor)
ctx.setLineWidth(2)
let wave = CGMutablePath()
wave.move(to: point(0, 552))
wave.addCurve(to: point(520, 566), control1: point(210, 504), control2: point(342, 614))
wave.addCurve(to: point(1200, 504), control1: point(704, 516), control2: point(1000, 430))
ctx.addPath(wave)
ctx.strokePath()

func line(_ a: CGPoint, _ b: CGPoint, alpha: CGFloat) {
    ctx.setStrokeColor(color(27, 104, 80, alpha).cgColor)
    ctx.setLineWidth(1.6)
    ctx.move(to: point(a.x, a.y))
    ctx.addLine(to: point(b.x, b.y))
    ctx.strokePath()
}

for (a, b, alpha) in [
    (CGPoint(x: 706, y: 156), CGPoint(x: 1034, y: 108), CGFloat(0.24)),
    (CGPoint(x: 674, y: 298), CGPoint(x: 1080, y: 230), CGFloat(0.22)),
    (CGPoint(x: 738, y: 428), CGPoint(x: 1136, y: 362), CGFloat(0.2))
] {
    line(a, b, alpha: alpha)
}

for point in [CGPoint(x: 706, y: 156), CGPoint(x: 878, y: 130), CGPoint(x: 1034, y: 108), CGPoint(x: 674, y: 298), CGPoint(x: 896, y: 262), CGPoint(x: 1080, y: 230), CGPoint(x: 738, y: 428), CGPoint(x: 948, y: 392), CGPoint(x: 1136, y: 362)] {
    ctx.setFillColor(color(27, 104, 80, 0.72).cgColor)
    ctx.fillEllipse(in: ellipse(point.x, point.y, 4))
}

for radius in stride(from: CGFloat(104), through: CGFloat(56), by: -16) {
    ctx.setFillColor(color(31, 91, 71, 0.055).cgColor)
    ctx.fillEllipse(in: ellipse(936, 214, radius))
}
ctx.setFillColor(color(216, 187, 119, 0.94).cgColor)
ctx.fillEllipse(in: rect(876, 154, 128, 128))
ctx.setFillColor(color(246, 241, 229, 0.97).cgColor)
ctx.fillEllipse(in: rect(916, 138, 132, 132))

let card = roundedRect(72, 70, 1056, 535, 28)
color(255, 252, 244, 0.78).setFill()
card.fill()
color(38, 86, 68, 0.18).setStroke()
card.lineWidth = 1
card.stroke()

drawText("XUNYUE NOTES", x: 128, y: 134, size: 22, weight: .medium, color: color(132, 94, 38), name: "Menlo", tracking: 4)
drawText("AI · Web3", x: 128, y: 210, size: 74, weight: .bold, color: color(18, 58, 48))
drawText("最新日报", x: 128, y: 294, size: 74, weight: .bold, color: color(18, 58, 48))

ctx.setFillColor(color(31, 104, 75, 0.72).cgColor)
ctx.fill(rect(130, 356, 360, 2))

drawText("每日追踪 AI、Web3 与开源技术信号", x: 130, y: 402, size: 30, weight: .medium, color: color(40, 84, 69))
drawText("DAILY BRIEF · AI / WEB3 / OPEN SOURCE", x: 130, y: 460, size: 21, weight: .regular, color: color(88, 124, 103), name: "Menlo", tracking: 2)

let author = roundedRect(130, 512, 224, 50, 25)
color(228, 201, 139, 0.92).setFill()
author.fill()
color(132, 94, 38, 0.34).setStroke()
author.lineWidth = 1.3
author.stroke()
drawText("寻月隐君", x: 162, y: 546, size: 24, weight: .medium, color: color(18, 58, 48))

let index = roundedRect(800, 418, 248, 102, 18)
color(255, 252, 244, 0.72).setFill()
index.fill()
color(132, 94, 38, 0.34).setStroke()
index.lineWidth = 1.1
index.stroke()
drawText("SIGNAL INDEX", x: 826, y: 458, size: 18, weight: .regular, color: color(132, 94, 38), name: "Menlo", tracking: 2)
drawText("AI  WEB3  AGENT  ZK", x: 826, y: 492, size: 17, weight: .regular, color: color(31, 91, 71), name: "Menlo")

ctx.setStrokeColor(color(18, 58, 48, 0.12).cgColor)
ctx.setLineWidth(2)
ctx.stroke(CGRect(x: 1, y: 1, width: width - 2, height: height - 2))

image.unlockFocus()

guard
    let tiff = image.tiffRepresentation,
    let bitmap = NSBitmapImageRep(data: tiff),
    let png = bitmap.representation(using: .png, properties: [:])
else {
    fatalError("Could not encode PNG")
}

let output = CommandLine.arguments.dropFirst().first ?? "docs/assets/wechat/ai-web3-daily-cover.png"
try png.write(to: URL(fileURLWithPath: output), options: .atomic)
