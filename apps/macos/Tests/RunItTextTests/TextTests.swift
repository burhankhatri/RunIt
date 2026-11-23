import RunItText
import XCTest

final class TextTests: XCTestCase {
    func testRevealNeverRunsPastTheEnd() {
        for count in [0, 1, 4, 5, 6, 40] {
            for step in 0..<300 {
                let r = Reveal(shown: Double(step) * 0.25, count: count)
                XCTAssertTrue(r.solid <= r.fading.lowerBound && r.fading.upperBound <= count)
            }
        }
        XCTAssertEqual(Reveal(shown: 500, count: 10).solid, 10)
        XCTAssertNil(Reveal(shown: 10, count: 10).caret)
        XCTAssertEqual(Reveal(shown: 3.5, count: 10).caret, 3)
    }

    func testRevealFadesInOrder() {
        let r = Reveal(shown: 8.5, count: 20)
        XCTAssertEqual(r.fading, 3..<8)
        XCTAssertEqual(r.fading.map { r.opacity($0) }, r.fading.map { r.opacity($0) }.sorted(by: >))
        XCTAssertTrue(r.fading.allSatisfy { r.opacity($0) > 0 && r.opacity($0) <= 1 })
    }

    func testSamplesKeepEveryCharacter() {
        XCTAssertEqual(Samples.split("a fairly extraordinary word\n  x").joined(), "a fairly extraordinary word\n  x")
        XCTAssertGreaterThan(Samples.prose.count, 60)
        XCTAssertTrue(Samples.prose.allSatisfy { $0.count <= 8 })
    }

    func testFigures() {
        XCTAssertEqual(Show.speed(9.94), "9.9")
        XCTAssertEqual(Show.speed(50.2), "50")
        XCTAssertEqual(Show.size(2_019_377_600), "2.0 GB")
        XCTAssertEqual(Show.license("unknown"), "licence not stated")
        XCTAssertEqual(Show.left(60_000_000, at: 1_000_000), "60 s left")
        XCTAssertEqual(Show.left(1, at: 0), "")
    }
}
