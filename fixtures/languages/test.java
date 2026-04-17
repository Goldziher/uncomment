// This line comment should be removed

/* This block comment should be removed */

/**
 * Javadoc comment that should be preserved.
 */
@SuppressWarnings("unused")
public class Test {
    public static void main(String[] args) {
        String url = "http://example.com//path";
        String msg = "Hello // world";
        // TODO: add logging
        System.out.println(msg);
    }
}
