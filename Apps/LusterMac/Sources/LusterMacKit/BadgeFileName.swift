import Foundation

/// The stage caption and the suggested export filename. Both derive from one
/// string, so the failure suffix is defined once.
public enum BadgeFileName {

    /// The caption before any SVG is loaded: none.
    public static let placeholder = ""

    /// The caption for a file that could not be used.
    public static func failureCaption(for name: String) -> String {
        "\(name)\(failureSuffix)"
    }

    /// The save panel's default name: the caption without its failure note
    /// or extension.
    public static func exportBase(from caption: String) -> String {
        var name = caption
        if let note = name.range(of: failureSuffix) { name.removeSubrange(note.lowerBound...) }
        name = name.trimmingCharacters(in: .whitespaces)
        // `deletingPathExtension` keeps a name like ".svg" whole; drop the
        // leading dots so the suggested file is never hidden.
        let stem = (name as NSString).deletingPathExtension
            .drop(while: { $0 == "." })
        return stem.isEmpty ? "badge" : String(stem)
    }

    private static var failureSuffix: String {
        "  (\(String(localized: "unreadable", comment: "After a file name that failed")))"
    }
}
