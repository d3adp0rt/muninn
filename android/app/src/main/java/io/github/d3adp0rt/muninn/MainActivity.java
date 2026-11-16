package io.github.d3adp0rt.muninn;

import android.os.Bundle;

import androidx.core.view.WindowCompat;

import com.google.androidgamesdk.GameActivity;

/**
 * The only Kotlin/Java in Muninn: Android glue.
 *
 * Everything else (UI, networking, storage, agent) lives in Rust.
 */
public class MainActivity extends GameActivity {
    static {
        System.loadLibrary("muninn");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // Keep content inside system bars for M0. Edge-to-edge lands with the
        // insets work in M1.
        WindowCompat.setDecorFitsSystemWindows(getWindow(), true);
        super.onCreate(savedInstanceState);
    }
}
