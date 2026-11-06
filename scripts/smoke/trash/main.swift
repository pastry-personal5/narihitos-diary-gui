// Synthetic AAC writer and the actual pinned producer's catalog/reopen entry point.
import Foundation
import AVFoundation

@main struct TrashProducerHarness {
  static func main() async throws {
    let args = CommandLine.arguments
    let root = URL(fileURLWithPath: args[2])
    if args[1] == "audio" {
      try makeAudio(root)
      return
    }
    let session = CatalogSessionID()
    var cursor = CatalogCursor(session: session, token: UUID())
    var registry = CatalogAssetRegistry()
    registry.register(DiaryCatalogHandler())
    let worker = FileCatalogWorker(session: session, cursor: cursor, registry: registry)
    var summaries: [[String: Any]] = []
    var recovery: [[String: Any]] = []
    var examined = 0
    for _ in 0..<1000 {
      let page = try await worker.page(cursor, root: root)
      examined += page.examinedCandidates
      for row in page.summaries {
        let reopened = try await worker.detail(row.handle, root: root)
        guard let detail = reopened.payload as? DiaryCatalogDetail else {
          fatalError("expected producer diary detail")
        }
        summaries.append(["asset_id": row.assetID ?? "", "body": detail.body,
                          "audio_available": detail.audioAvailable])
      }
      for row in page.recovery {
        recovery.append(["issue": row.issue.rawValue, "has_text": row.hasText,
                         "has_audio": row.hasAudio])
      }
      guard let next = page.continuation else {
        let data = try JSONSerialization.data(withJSONObject:
          ["summaries": summaries, "recovery": recovery, "examined": examined], options: [.sortedKeys])
        print(String(decoding: data, as: UTF8.self))
        return
      }
      cursor = next
    }
    fatalError("catalog page bound")
  }
  static func makeAudio(_ url: URL) throws {
    let format = AVAudioFormat(standardFormatWithSampleRate: 44100, channels: 1)!
    let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 44100)!
    buffer.frameLength = 44100
    for i in 0..<44100 { buffer.floatChannelData![0][i] = Float(sin(Double(i) * 440 * 2 * .pi / 44100) * 0.1) }
    let writer = try AVAudioFile(forWriting: url, settings:
      [AVFormatIDKey: kAudioFormatMPEG4AAC, AVSampleRateKey: 44100,
       AVNumberOfChannelsKey: 1, AVEncoderBitRateKey: 64000],
      commonFormat: .pcmFormatFloat32, interleaved: false)
    try writer.write(from: buffer)
  }
}
