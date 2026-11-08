import Foundation

enum AssetValueError: String, Error, CustomStringConvertible, Sendable {
  case invalidAssetID
  case invalidIdentifier
  case invalidInstant
  case invalidBody

  var description: String { rawValue }
}

struct AssetID: Hashable, Sendable, CustomStringConvertible {
  let description: String

  init(_ value: String) throws {
    let bytes = Array(value.utf8)
    guard bytes.count == 36,
      bytes[8] == 45, bytes[13] == 45, bytes[18] == 45, bytes[23] == 45,
      bytes.enumerated().allSatisfy({ index, byte in
        [8, 13, 18, 23].contains(index) || (48...57).contains(byte) || (97...102).contains(byte)
      }),
      bytes[14] == 52,
      [56, 57, 97, 98].contains(bytes[19]),
      UUID(uuidString: value)?.uuidString.lowercased() == value
    else { throw AssetValueError.invalidAssetID }
    description = value
  }

  static func random() throws -> AssetID {
    // Foundation UUIDs are RFC 4122 version 4 on Apple platforms.
    try AssetID(UUID().uuidString.lowercased())
  }
}

struct StableIdentifier: Hashable, Sendable, CustomStringConvertible {
  let description: String

  init(_ value: String) throws {
    guard !value.isEmpty, value.utf8.count <= 8_192,
      value.unicodeScalars.allSatisfy({ !CharacterSet.controlCharacters.contains($0) })
    else { throw AssetValueError.invalidIdentifier }
    description = value
  }
}

typealias AssetKindID = StableIdentifier
typealias TranscriptionMethodID = StableIdentifier
typealias TitleOriginID = StableIdentifier

struct SchemaInstant: Hashable, Sendable, CustomStringConvertible {
  let description: String

  static func from(date: Date) throws -> SchemaInstant {
    let formatter = DateFormatter()
    formatter.calendar = Calendar(identifier: .gregorian)
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.timeZone = TimeZone(secondsFromGMT: 0)
    formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss'Z'"
    return try SchemaInstant(formatter.string(from: date))
  }

  init(_ value: String) throws {
    let bytes = Array(value.utf8)
    guard bytes.count == 20,
      bytes[4] == 45, bytes[7] == 45, bytes[10] == 84,
      bytes[13] == 58, bytes[16] == 58, bytes[19] == 90
    else { throw AssetValueError.invalidInstant }

    func number(_ range: Range<Int>) -> Int? {
      var result = 0
      for index in range {
        guard (48...57).contains(bytes[index]) else { return nil }
        result = result * 10 + Int(bytes[index] - 48)
      }
      return result
    }

    guard let year = number(0..<4), let month = number(5..<7), let day = number(8..<10),
      let hour = number(11..<13), let minute = number(14..<16), let second = number(17..<19),
      (0...23).contains(hour), (0...59).contains(minute), (0...59).contains(second)
    else { throw AssetValueError.invalidInstant }

    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(secondsFromGMT: 0)!
    var components = DateComponents()
    components.calendar = calendar
    components.timeZone = calendar.timeZone
    components.year = year
    components.month = month
    components.day = day
    components.hour = hour
    components.minute = minute
    components.second = second
    guard let date = calendar.date(from: components) else { throw AssetValueError.invalidInstant }
    let check = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
    guard check.year == year, check.month == month, check.day == day, check.hour == hour,
      check.minute == minute, check.second == second
    else { throw AssetValueError.invalidInstant }
    description = value
  }

  var year: String { String(description.prefix(4)) }
  var month: String { String(description.dropFirst(5).prefix(2)) }
  var day: String { String(description.dropFirst(8).prefix(2)) }
  var fileTimestamp: String {
    String(description.prefix(10)) + "_"
      + String(description.dropFirst(11).prefix(8)).replacingOccurrences(
        of: ":", with: "-")
  }
}

struct DiaryBody: Hashable, Sendable {
  static let maximumBytes = 1_048_576
  let text: String

  init(_ text: String) throws {
    guard text.utf8.count <= Self.maximumBytes else { throw AssetValueError.invalidBody }
    self.text = text
  }
}

struct TranscriptionProvenance: Equatable, Sendable {
  let methodID: TranscriptionMethodID
  let languageHint: String
  let settingsRevision: Int64
  let modelID: String?
  let modelVersion: String?
  let runtimeID: String?
  let conversionID: String?
}

struct TitleProvenance: Equatable, Sendable {
  let originID: TitleOriginID
  let promptRevision: String?
  let osVersion: String?
  let modelID: String?
}

struct LocationContext: Equatable, Sendable {
  let latitude: Decimal
  let longitude: Decimal
  let sampledAt: SchemaInstant
  let horizontalAccuracyMeters: Decimal
  let placeName: String?
}

struct WeatherContext: Equatable, Sendable {
  let observedAt: SchemaInstant
  let summary: String
  let temperatureCelsius: Decimal
}

struct PreservedScalar: Equatable, Sendable {
  let key: String
  let rawValue: String
  let sourceOrder: Int
}

enum DiaryCreationMethod: String, Sendable {
  case recording
  case manual
}

struct DiarySnippetV1: Equatable, Sendable {
  let id: AssetID
  let createdAt: SchemaInstant
  var updatedAt: SchemaInstant
  var title: String
  var titleProvenance: TitleProvenance
  var favorite: Bool
  let language: String?
  let audioFilename: String?
  var audioSHA256: String?
  let recordingDurationMilliseconds: Int64?
  let audioCodec: String?
  let audioSampleRateHertz: Int64?
  let audioChannels: Int64?
  let audioBitrateBPS: Int64?
  let transcription: TranscriptionProvenance?
  let location: LocationContext?
  let weather: WeatherContext?
  var body: DiaryBody
  var preservedScalars: [PreservedScalar]
  var creationMethod: DiaryCreationMethod = .recording

  func withPreservedScalars(_ scalars: [PreservedScalar]) -> Self {
    var result = self
    result.preservedScalars = scalars
    return result
  }

  static func manual(
    id: AssetID, createdAt: SchemaInstant, body: DiaryBody, title: String,
    provenance: TitleProvenance
  ) -> Self {
    return Self(
      id: id, createdAt: createdAt, updatedAt: createdAt, title: title,
      titleProvenance: provenance,
      favorite: false, language: nil, audioFilename: nil, audioSHA256: nil,
      recordingDurationMilliseconds: nil, audioCodec: nil, audioSampleRateHertz: nil,
      audioChannels: nil, audioBitrateBPS: nil, transcription: nil, location: nil, weather: nil,
      body: body, preservedScalars: [], creationMethod: .manual)
  }
}

enum DiarySnippetSupportPolicy {
  static let schemaVersion: Int64 = 1
  static let kind = "ndm.diary-snippet"
  static let transcriptionMethods = ["ndm.transcription.apple", "ndm.transcription.cohere"]
  static let titleOrigins = ["apple-ai", "first-sentence", "empty-body"]

  static func supportsTranscriptionSettings(revision: Int64, method: String) -> Bool {
    revision == 1 || (revision == 2 && method == "ndm.transcription.cohere")
  }
}
