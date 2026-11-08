import Foundation

struct DiarySnippetLayout: Equatable, Sendable {
  let directoryComponents: [String]
  let markdownFilename: String
  let assetsDirectoryName: String
  let audioFilename: String

  init(id: AssetID, createdAt: SchemaInstant) {
    directoryComponents = [createdAt.year, createdAt.month, createdAt.day]
    let stem = "\(createdAt.fileTimestamp)-\(id.description)"
    markdownFilename = "\(stem).md"
    assetsDirectoryName = "\(stem).assets"
    audioFilename = "\(id.description).m4a"
  }

  func validates(audioReference: String?) -> Bool {
    guard let audioReference else { return false }
    return audioReference == audioFilename && !audioReference.contains("/")
      && !audioReference.contains("\\")
      && !audioReference.contains(":") && !audioReference.contains("%")
      && !audioReference.unicodeScalars.contains(where: {
        CharacterSet.controlCharacters.contains($0)
      })
  }
}
