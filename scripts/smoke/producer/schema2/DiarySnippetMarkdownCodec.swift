import Foundation

// swift-format's canonical multiline collections conflict with SwiftLint's trailing-comma rule.
// swiftlint:disable file_length trailing_comma

enum DiarySnippetMalformedCode: String, CaseIterable, Error, CustomStringConvertible, Sendable {
  case malformedUTF8
  case invalidLineEnding
  case missingDelimiter
  case frontMatterTooLarge
  case bodyTooLarge
  case tooManyKeys
  case scalarTooLarge
  case invalidKey
  case duplicateKey
  case forbiddenSyntax
  case missingRequiredField
  case invalidField
  case unsafeAudioReference

  var description: String { rawValue }
}

enum DiarySnippetUnsupportedReason: Equatable, Sendable {
  case schemaVersion
  case kind
  case transcriptionMethod
  case titleOrigin
}

enum DiarySnippetDecodingResult: Equatable, Sendable {
  case supported(DiarySnippetV1)
  case unsupported(reason: DiarySnippetUnsupportedReason, original: Data)
  case malformed(DiarySnippetMalformedCode)
}

// The bounded grammar is intentionally centralized so decode and encode share one allowlist.
// swiftlint:disable:next type_body_length
struct DiarySnippetMarkdownCodec: Sendable {
  static let maximumFrontMatterBytes = 65_536
  static let maximumKeys = 64
  static let maximumScalarBytes = 8_192

  private enum Scalar {
    case string(String)
    case integer(Int64)
    case decimal(Decimal)
    case boolean(Bool)
  }

  private struct Entry {
    let key: String
    let raw: String
    let value: Scalar
    let order: Int
  }

  private static let requiredKeys = [
    "schema_version", "kind", "id", "created_at", "updated_at", "title", "title_origin",
    "favorite", "language", "audio_filename", "recording_duration_ms", "audio_codec",
    "audio_sample_rate_hz", "audio_channels", "audio_bitrate_bps", "transcription_method_id",
    "transcription_language_hint", "transcription_settings_revision",
  ]

  private static let conditionalKeys = [
    "audio_sha256",
    "transcription_model_id", "transcription_model_version", "transcription_runtime_id",
    "transcription_conversion_id", "title_prompt_revision", "title_os_version", "title_model_id",
  ]

  private static let locationKeys = [
    "location_latitude", "location_longitude", "location_sampled_at",
    "location_horizontal_accuracy_m", "location_place_name",
  ]

  private static let weatherKeys = [
    "weather_observed_at", "weather_summary", "weather_temperature_celsius",
  ]

  private static let recognizedKeys = Set(
    requiredKeys + conditionalKeys + locationKeys + weatherKeys)

  // swiftlint:disable:next cyclomatic_complexity function_body_length
  func decode(_ data: Data) -> DiarySnippetDecodingResult {
    do {
      let parsed = try parse(data)
      let entries = parsed.entries

      if try integer("schema_version", in: entries) == 2 {
        return .supported(try decodeManual(entries, body: parsed.body))
      }
      guard try integer("schema_version", in: entries) == DiarySnippetSupportPolicy.schemaVersion
      else {
        return .unsupported(reason: .schemaVersion, original: data)
      }
      guard try string("kind", in: entries) == DiarySnippetSupportPolicy.kind else {
        return .unsupported(reason: .kind, original: data)
      }
      let method = try string("transcription_method_id", in: entries)
      guard DiarySnippetSupportPolicy.transcriptionMethods.contains(method) else {
        return .unsupported(reason: .transcriptionMethod, original: data)
      }
      let origin = try string("title_origin", in: entries)
      guard DiarySnippetSupportPolicy.titleOrigins.contains(origin) else {
        return .unsupported(reason: .titleOrigin, original: data)
      }

      let id = try AssetID(string("id", in: entries))
      let createdAt = try SchemaInstant(string("created_at", in: entries))
      let updatedAt = try SchemaInstant(string("updated_at", in: entries))
      let audioFilename = try string("audio_filename", in: entries)
      let audioSHA256 = optionalString("audio_sha256", in: entries)
      guard audioSHA256.map(Self.validSHA256) ?? true else {
        throw DiarySnippetMalformedCode.invalidField
      }
      guard
        DiarySnippetLayout(id: id, createdAt: createdAt).validates(audioReference: audioFilename)
      else {
        throw DiarySnippetMalformedCode.unsafeAudioReference
      }

      let duration = try integer("recording_duration_ms", in: entries)
      let sampleRate = try integer("audio_sample_rate_hz", in: entries)
      let channels = try integer("audio_channels", in: entries)
      let bitrate = try integer("audio_bitrate_bps", in: entries)
      let settingsRevision = try integer("transcription_settings_revision", in: entries)
      guard (0...301_000).contains(duration), sampleRate > 0, channels > 0, bitrate > 0,
        DiarySnippetSupportPolicy.supportsTranscriptionSettings(
          revision: settingsRevision, method: method)
      else { throw DiarySnippetMalformedCode.invalidField }

      let titleOriginID = try TitleOriginID(origin)
      let title = try string("title", in: entries)
      guard title.count <= 30 else { throw DiarySnippetMalformedCode.invalidField }
      for key in Self.conditionalKeys where entries[key] != nil {
        guard optionalString(key, in: entries) != nil else {
          throw DiarySnippetMalformedCode.invalidField
        }
      }
      let promptRevision = optionalString("title_prompt_revision", in: entries)
      let osVersion = optionalString("title_os_version", in: entries)
      let titleModelID = optionalString("title_model_id", in: entries)
      if origin == "apple-ai" {
        guard promptRevision != nil, osVersion != nil else {
          throw DiarySnippetMalformedCode.invalidField
        }
      } else if promptRevision != nil || osVersion != nil || titleModelID != nil {
        throw DiarySnippetMalformedCode.invalidField
      }

      let context = parseContext(entries)
      let body = try DiaryBody(parsed.body)
      let knownOptionalContext = Set(Self.locationKeys + Self.weatherKeys)
      let preserved = entries.values.compactMap { entry -> PreservedScalar? in
        let unknown = !Self.recognizedKeys.contains(entry.key)
        let uninterpretedContext =
          knownOptionalContext.contains(entry.key)
          && (!context.locationIsValid && Self.locationKeys.contains(entry.key)
            || !context.weatherIsValid && Self.weatherKeys.contains(entry.key))
        guard unknown || uninterpretedContext else { return nil }
        return PreservedScalar(key: entry.key, rawValue: entry.raw, sourceOrder: entry.order)
      }.sorted { $0.sourceOrder < $1.sourceOrder }

      return .supported(
        DiarySnippetV1(
          id: id,
          createdAt: createdAt,
          updatedAt: updatedAt,
          title: title,
          titleProvenance: TitleProvenance(
            originID: titleOriginID, promptRevision: promptRevision, osVersion: osVersion,
            modelID: titleModelID),
          favorite: try boolean("favorite", in: entries),
          language: try string("language", in: entries),
          audioFilename: audioFilename,
          audioSHA256: audioSHA256,
          recordingDurationMilliseconds: duration,
          audioCodec: try string("audio_codec", in: entries),
          audioSampleRateHertz: sampleRate,
          audioChannels: channels,
          audioBitrateBPS: bitrate,
          transcription: TranscriptionProvenance(
            methodID: try TranscriptionMethodID(method),
            languageHint: try string("transcription_language_hint", in: entries),
            settingsRevision: settingsRevision,
            modelID: optionalString("transcription_model_id", in: entries),
            modelVersion: optionalString("transcription_model_version", in: entries),
            runtimeID: optionalString("transcription_runtime_id", in: entries),
            conversionID: optionalString("transcription_conversion_id", in: entries)),
          location: context.location,
          weather: context.weather,
          body: body,
          preservedScalars: preserved))
    } catch let code as DiarySnippetMalformedCode {
      return .malformed(code)
    } catch {
      return .malformed(.invalidField)
    }
  }

  // swiftlint:disable:next function_body_length
  func encode(_ snippet: DiarySnippetV1) throws -> Data {
    if snippet.creationMethod == .manual { return try encodeManual(snippet) }
    try validateForEncoding(snippet)
    var lines: [String] = ["---"]
    func quoted(_ key: String, _ value: String) { lines.append("\(key): \(Self.quote(value))") }
    func integer(_ key: String, _ value: Int64) { lines.append("\(key): \(value)") }

    integer("schema_version", 1)
    quoted("kind", DiarySnippetSupportPolicy.kind)
    quoted("id", snippet.id.description)
    quoted("created_at", snippet.createdAt.description)
    quoted("updated_at", snippet.updatedAt.description)
    quoted("title", snippet.title)
    quoted("title_origin", snippet.titleProvenance.originID.description)
    lines.append("favorite: \(snippet.favorite ? "true" : "false")")
    quoted("language", snippet.language!)
    quoted("audio_filename", snippet.audioFilename!)
    appendOptional("audio_sha256", snippet.audioSHA256, to: &lines)
    integer("recording_duration_ms", snippet.recordingDurationMilliseconds!)
    quoted("audio_codec", snippet.audioCodec!)
    integer("audio_sample_rate_hz", snippet.audioSampleRateHertz!)
    integer("audio_channels", snippet.audioChannels!)
    integer("audio_bitrate_bps", snippet.audioBitrateBPS!)
    quoted("transcription_method_id", snippet.transcription!.methodID.description)
    quoted("transcription_language_hint", snippet.transcription!.languageHint)
    integer("transcription_settings_revision", snippet.transcription!.settingsRevision)
    appendOptional("transcription_model_id", snippet.transcription!.modelID, to: &lines)
    appendOptional("transcription_model_version", snippet.transcription!.modelVersion, to: &lines)
    appendOptional("transcription_runtime_id", snippet.transcription!.runtimeID, to: &lines)
    appendOptional("transcription_conversion_id", snippet.transcription!.conversionID, to: &lines)
    appendOptional("title_prompt_revision", snippet.titleProvenance.promptRevision, to: &lines)
    appendOptional("title_os_version", snippet.titleProvenance.osVersion, to: &lines)
    appendOptional("title_model_id", snippet.titleProvenance.modelID, to: &lines)
    if let location = snippet.location {
      lines.append("location_latitude: \(canonical(location.latitude))")
      lines.append("location_longitude: \(canonical(location.longitude))")
      quoted("location_sampled_at", location.sampledAt.description)
      lines.append(
        "location_horizontal_accuracy_m: \(canonical(location.horizontalAccuracyMeters))")
      appendOptional("location_place_name", location.placeName, to: &lines)
    }
    if let weather = snippet.weather {
      quoted("weather_observed_at", weather.observedAt.description)
      quoted("weather_summary", weather.summary)
      lines.append("weather_temperature_celsius: \(canonical(weather.temperatureCelsius))")
    }
    for scalar in snippet.preservedScalars.sorted(by: { $0.sourceOrder < $1.sourceOrder }) {
      guard scalar.key.utf8.count <= Self.maximumScalarBytes,
        scalar.rawValue.utf8.count <= Self.maximumScalarBytes,
        Self.validKey(scalar.key),
        !Self.recognizedKeys.subtracting(Self.locationKeys + Self.weatherKeys).contains(scalar.key)
      else { throw DiarySnippetMalformedCode.invalidField }
      lines.append("\(scalar.key): \(scalar.rawValue)")
    }
    guard lines.count - 1 <= Self.maximumKeys else {
      throw DiarySnippetMalformedCode.tooManyKeys
    }
    let frontMatter = lines.joined(separator: "\n")
    guard frontMatter.utf8.count - 4 <= Self.maximumFrontMatterBytes else {
      throw DiarySnippetMalformedCode.frontMatterTooLarge
    }
    lines.append("---")
    lines.append("")
    let output = lines.joined(separator: "\n") + "\n" + snippet.body.text
    let data = Data(output.utf8)
    guard case .supported = decode(data) else { throw DiarySnippetMalformedCode.invalidField }
    return data
  }

  private func validateForEncoding(_ snippet: DiarySnippetV1) throws {
    guard snippet.creationMethod == .recording,
      let audioFilename = snippet.audioFilename,
      let duration = snippet.recordingDurationMilliseconds,
      let sampleRate = snippet.audioSampleRateHertz, let channels = snippet.audioChannels,
      let bitrate = snippet.audioBitrateBPS, snippet.audioCodec != nil, snippet.language != nil,
      snippet.transcription != nil
    else { throw DiarySnippetMalformedCode.invalidField }
    let strings = [
      snippet.title, snippet.language, snippet.audioFilename, snippet.audioSHA256,
      snippet.audioCodec,
      snippet.transcription!.methodID.description, snippet.transcription!.languageHint,
      snippet.transcription!.modelID, snippet.transcription!.modelVersion,
      snippet.transcription!.runtimeID, snippet.transcription!.conversionID,
      snippet.titleProvenance.originID.description, snippet.titleProvenance.promptRevision,
      snippet.titleProvenance.osVersion, snippet.titleProvenance.modelID,
      snippet.location?.placeName, snippet.weather?.summary,
    ].compactMap { $0 }
    guard strings.allSatisfy({ Self.quote($0).utf8.count <= Self.maximumScalarBytes }),
      snippet.title.count <= 30,
      DiarySnippetSupportPolicy.transcriptionMethods.contains(
        snippet.transcription!.methodID.description),
      DiarySnippetSupportPolicy.titleOrigins.contains(snippet.titleProvenance.originID.description),
      DiarySnippetSupportPolicy.supportsTranscriptionSettings(
        revision: snippet.transcription!.settingsRevision,
        method: snippet.transcription!.methodID.description),
      (0...301_000).contains(duration),
      sampleRate > 0, channels > 0, bitrate > 0,
      DiarySnippetLayout(id: snippet.id, createdAt: snippet.createdAt).validates(
        audioReference: audioFilename),
      snippet.audioSHA256.map(Self.validSHA256) ?? true
    else { throw DiarySnippetMalformedCode.invalidField }
  }

  private func decodeManual(_ entries: [String: Entry], body: String) throws -> DiarySnippetV1 {
    guard try string("kind", in: entries) == DiarySnippetSupportPolicy.kind,
      try string("creation_method", in: entries) == "manual"
    else { throw DiarySnippetMalformedCode.invalidField }
    let forbidden = Set(
      Self.requiredKeys.dropFirst(8) + ["audio_sha256"]
        + Self.conditionalKeys.filter { $0.hasPrefix("transcription_") }
        + Self.locationKeys + Self.weatherKeys)
    guard forbidden.allSatisfy({ entries[$0] == nil }) else {
      throw DiarySnippetMalformedCode.invalidField
    }
    let origin = try string("title_origin", in: entries)
    let title = try string("title", in: entries)
    guard DiarySnippetSupportPolicy.titleOrigins.contains(origin), title.count <= 30 else {
      throw DiarySnippetMalformedCode.invalidField
    }
    let titleKeys = ["title_prompt_revision", "title_os_version", "title_model_id"]
    for key in titleKeys where entries[key] != nil {
      guard optionalString(key, in: entries) != nil else {
        throw DiarySnippetMalformedCode.invalidField
      }
    }
    let prompt = optionalString("title_prompt_revision", in: entries)
    let osVersion = optionalString("title_os_version", in: entries)
    let model = optionalString("title_model_id", in: entries)
    guard
      origin == "apple-ai"
        ? prompt != nil && osVersion != nil : prompt == nil && osVersion == nil && model == nil
    else { throw DiarySnippetMalformedCode.invalidField }
    var result = DiarySnippetV1.manual(
      id: try AssetID(string("id", in: entries)),
      createdAt: try SchemaInstant(string("created_at", in: entries)), body: try DiaryBody(body),
      title: title,
      provenance: TitleProvenance(
        originID: try TitleOriginID(origin), promptRevision: prompt, osVersion: osVersion,
        modelID: model))
    result.updatedAt = try SchemaInstant(string("updated_at", in: entries))
    result.favorite = try boolean("favorite", in: entries)
    return result.withPreservedScalars(
      entries.values.filter {
        !Self.recognizedKeys.contains($0.key) && $0.key != "creation_method"
      }.sorted { $0.order < $1.order }.map {
        PreservedScalar(key: $0.key, rawValue: $0.raw, sourceOrder: $0.order)
      })
  }

  private func encodeManual(_ snippet: DiarySnippetV1) throws -> Data {
    guard snippet.audioFilename == nil, snippet.audioSHA256 == nil,
      snippet.transcription == nil, snippet.language == nil, snippet.location == nil,
      snippet.weather == nil, snippet.recordingDurationMilliseconds == nil,
      snippet.audioCodec == nil, snippet.audioSampleRateHertz == nil,
      snippet.audioChannels == nil, snippet.audioBitrateBPS == nil
    else { throw DiarySnippetMalformedCode.invalidField }
    var lines = [
      "---", "schema_version: 2", "kind: \(Self.quote(DiarySnippetSupportPolicy.kind))",
      "creation_method: \"manual\"", "id: \(Self.quote(snippet.id.description))",
      "created_at: \(Self.quote(snippet.createdAt.description))",
      "updated_at: \(Self.quote(snippet.updatedAt.description))",
      "title: \(Self.quote(snippet.title))",
      "title_origin: \(Self.quote(snippet.titleProvenance.originID.description))",
      "favorite: \(snippet.favorite ? "true" : "false")",
    ]
    appendOptional("title_prompt_revision", snippet.titleProvenance.promptRevision, to: &lines)
    appendOptional("title_os_version", snippet.titleProvenance.osVersion, to: &lines)
    appendOptional("title_model_id", snippet.titleProvenance.modelID, to: &lines)
    for scalar in snippet.preservedScalars.sorted(by: { $0.sourceOrder < $1.sourceOrder }) {
      guard !Self.recognizedKeys.contains(scalar.key), scalar.key != "creation_method" else {
        throw DiarySnippetMalformedCode.invalidField
      }
      lines.append("\(scalar.key): \(scalar.rawValue)")
    }
    let data = Data((lines.joined(separator: "\n") + "\n---\n\n" + snippet.body.text).utf8)
    guard case .supported = decode(data) else {
      throw DiarySnippetMalformedCode.invalidField
    }
    return data
  }

  private static func validSHA256(_ value: String) -> Bool {
    value.utf8.count == 64
      && value.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
  }

  private func parse(_ data: Data) throws -> (entries: [String: Entry], body: String) {
    let delimiterBytes = 10  // opening `---\n` plus closing `\n---\n\n`
    let maximumDocumentBytes =
      Self.maximumFrontMatterBytes + DiaryBody.maximumBytes + delimiterBytes
    guard data.count <= maximumDocumentBytes else { throw DiarySnippetMalformedCode.bodyTooLarge }
    guard let source = String(data: data, encoding: .utf8) else {
      throw DiarySnippetMalformedCode.malformedUTF8
    }
    guard source.hasPrefix("---\n") else {
      throw data.contains(13)
        ? DiarySnippetMalformedCode.invalidLineEnding : DiarySnippetMalformedCode.missingDelimiter
    }
    guard
      let close = source.range(
        of: "\n---\n\n", range: source.index(source.startIndex, offsetBy: 3)..<source.endIndex)
    else {
      throw data.contains(13)
        ? DiarySnippetMalformedCode.invalidLineEnding : DiarySnippetMalformedCode.missingDelimiter
    }
    let front = String(source[source.index(source.startIndex, offsetBy: 4)..<close.lowerBound])
    guard !front.contains("\r") else { throw DiarySnippetMalformedCode.invalidLineEnding }
    guard front.utf8.count <= Self.maximumFrontMatterBytes else {
      throw DiarySnippetMalformedCode.frontMatterTooLarge
    }
    let bodyStart = close.upperBound
    let body = String(source[bodyStart...])
    guard body.utf8.count <= DiaryBody.maximumBytes else {
      throw DiarySnippetMalformedCode.bodyTooLarge
    }
    let lines = front.isEmpty ? [] : front.split(separator: "\n", omittingEmptySubsequences: false)
    guard lines.count <= Self.maximumKeys else { throw DiarySnippetMalformedCode.tooManyKeys }
    let entries = try parseEntries(lines)
    if (try? integer("schema_version", in: entries)) != 2, data.contains(13) {
      throw DiarySnippetMalformedCode.invalidLineEnding
    }
    let baseKeys = Array(Self.requiredKeys.prefix(8))
    let required =
      (try? integer("schema_version", in: entries)) == 2
      ? baseKeys + ["creation_method"] : Self.requiredKeys
    guard required.allSatisfy({ entries[$0] != nil }) else {
      throw DiarySnippetMalformedCode.missingRequiredField
    }
    return (entries, body)
  }

  private func parseEntries(_ lines: [Substring]) throws -> [String: Entry] {
    var entries: [String: Entry] = [:]
    for (order, lineSlice) in lines.enumerated() {
      let line = String(lineSlice)
      guard let separator = line.range(of: ": ") else {
        throw DiarySnippetMalformedCode.forbiddenSyntax
      }
      let key = String(line[..<separator.lowerBound])
      let raw = String(line[separator.upperBound...])
      guard Self.validKey(key) else { throw DiarySnippetMalformedCode.invalidKey }
      guard entries[key] == nil else { throw DiarySnippetMalformedCode.duplicateKey }
      guard raw.utf8.count <= Self.maximumScalarBytes else {
        throw DiarySnippetMalformedCode.scalarTooLarge
      }
      entries[key] = Entry(key: key, raw: raw, value: try Self.parseScalar(raw), order: order)
    }
    return entries
  }

  private static func validKey(_ key: String) -> Bool {
    guard let first = key.utf8.first, (97...122).contains(first) else { return false }
    return key.utf8.allSatisfy { (97...122).contains($0) || (48...57).contains($0) || $0 == 95 }
  }

  private static func parseScalar(_ raw: String) throws -> Scalar {
    if raw.hasPrefix("\"") { return .string(try unquote(raw)) }
    if raw == "true" { return .boolean(true) }
    if raw == "false" { return .boolean(false) }
    guard !raw.isEmpty,
      !raw.contains(where: { $0.isWhitespace }),
      !raw.contains(":"), !raw.contains("["), !raw.contains("]"),
      !raw.contains("{") && !raw.contains("}"),
      !raw.hasPrefix("&"), !raw.hasPrefix("*"), !raw.hasPrefix("!"),
      !raw.hasPrefix("|") && !raw.hasPrefix(">")
    else { throw DiarySnippetMalformedCode.forbiddenSyntax }
    if !raw.contains(".") {
      guard let value = Int64(raw), String(value) == raw || (value == 0 && raw == "0") else {
        throw DiarySnippetMalformedCode.invalidField
      }
      return .integer(value)
    }
    let decimalDigitCount = raw.utf8.lazy.filter { (48...57).contains($0) }.count
    guard decimalDigitCount <= 38,
      raw.range(of: #"^-?(0|[1-9][0-9]*)\.[0-9]+$"#, options: .regularExpression) != nil,
      let decimal = Decimal(string: raw, locale: Locale(identifier: "en_US_POSIX")),
      NSDecimalNumber(decimal: decimal) != .notANumber
    else { throw DiarySnippetMalformedCode.invalidField }
    return .decimal(decimal)
  }

  // swiftlint:disable:next cyclomatic_complexity
  private static func unquote(_ raw: String) throws -> String {
    guard raw.count >= 2, raw.last == "\"" else { throw DiarySnippetMalformedCode.invalidField }
    var result = ""
    var escaped = false
    for character in raw.dropFirst().dropLast() {
      if escaped {
        switch character {
        case "\"": result.append("\"")
        case "\\": result.append("\\")
        case "n": result.append("\n")
        case "r": result.append("\r")
        case "t": result.append("\t")
        default: throw DiarySnippetMalformedCode.invalidField
        }
        escaped = false
      } else if character == "\\" {
        escaped = true
      } else {
        guard character != "\"", !character.isNewline,
          character.unicodeScalars.allSatisfy({ !CharacterSet.controlCharacters.contains($0) })
        else { throw DiarySnippetMalformedCode.invalidField }
        result.append(character)
      }
    }
    guard !escaped else { throw DiarySnippetMalformedCode.invalidField }
    return result
  }

  private static func quote(_ value: String) -> String {
    var escaped = value.replacingOccurrences(of: "\\", with: "\\\\")
    escaped = escaped.replacingOccurrences(of: "\"", with: "\\\"")
    escaped = escaped.replacingOccurrences(of: "\n", with: "\\n")
    escaped = escaped.replacingOccurrences(of: "\r", with: "\\r")
    escaped = escaped.replacingOccurrences(of: "\t", with: "\\t")
    return "\"\(escaped)\""
  }

  private func string(_ key: String, in entries: [String: Entry]) throws -> String {
    guard case .string(let value) = entries[key]?.value else {
      throw DiarySnippetMalformedCode.invalidField
    }
    return value
  }

  private func optionalString(_ key: String, in entries: [String: Entry]) -> String? {
    guard case .string(let value) = entries[key]?.value else { return nil }
    return value
  }

  private func integer(_ key: String, in entries: [String: Entry]) throws -> Int64 {
    guard case .integer(let value) = entries[key]?.value else {
      throw DiarySnippetMalformedCode.invalidField
    }
    return value
  }

  private func boolean(_ key: String, in entries: [String: Entry]) throws -> Bool {
    guard case .boolean(let value) = entries[key]?.value else {
      throw DiarySnippetMalformedCode.invalidField
    }
    return value
  }

  private func decimal(_ key: String, in entries: [String: Entry]) -> Decimal? {
    guard case .decimal(let value) = entries[key]?.value else { return nil }
    return value
  }

  private struct ParsedContext {
    let location: LocationContext?
    let weather: WeatherContext?
    let locationIsValid: Bool
    let weatherIsValid: Bool
  }

  private func parseContext(_ entries: [String: Entry]) -> ParsedContext {
    let hasLocation = Self.locationKeys.contains { entries[$0] != nil }
    let hasWeather = Self.weatherKeys.contains { entries[$0] != nil }
    var location: LocationContext?
    let placeNameHasValidType =
      entries["location_place_name"] == nil
      || optionalString("location_place_name", in: entries) != nil
    // swift-format places braces after multiline condition lists; SwiftLint requires the opposite.
    // swiftlint:disable opening_brace
    if hasLocation, let latitude = decimal("location_latitude", in: entries),
      let longitude = decimal("location_longitude", in: entries),
      let accuracy = decimal("location_horizontal_accuracy_m", in: entries),
      let sampled = optionalString("location_sampled_at", in: entries).flatMap({
        try? SchemaInstant($0)
      }),
      latitude >= -90, latitude <= 90, longitude >= -180, longitude <= 180, accuracy >= 0,
      placeNameHasValidType
    {
      location = LocationContext(
        latitude: latitude, longitude: longitude, sampledAt: sampled,
        horizontalAccuracyMeters: accuracy,
        placeName: optionalString("location_place_name", in: entries))
    }
    var weather: WeatherContext?
    if hasWeather,
      let observed = optionalString("weather_observed_at", in: entries).flatMap({
        try? SchemaInstant($0)
      }),
      let summary = optionalString("weather_summary", in: entries),
      let temperature = decimal("weather_temperature_celsius", in: entries)
    {
      weather = WeatherContext(
        observedAt: observed, summary: summary, temperatureCelsius: temperature)
    }
    // swiftlint:enable opening_brace
    return ParsedContext(
      location: location, weather: weather, locationIsValid: !hasLocation || location != nil,
      weatherIsValid: !hasWeather || weather != nil)
  }

  private func appendOptional(_ key: String, _ value: String?, to lines: inout [String]) {
    if let value { lines.append("\(key): \(Self.quote(value))") }
  }

  private func canonical(_ value: Decimal) -> String {
    let spelling = NSDecimalNumber(decimal: value).stringValue
    return spelling.contains(".") ? spelling : "\(spelling).0"
  }
}

// swiftlint:enable file_length trailing_comma
