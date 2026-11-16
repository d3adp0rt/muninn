package io.github.d3adp0rt.muninn;

import android.graphics.Color;
import android.os.Bundle;
import android.view.View;

import androidx.core.graphics.Insets;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;

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

    private native void nativeInsets(int top, int bottom, int left, int right);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);

        getWindow().setStatusBarColor(Color.TRANSPARENT);
        getWindow().setNavigationBarColor(Color.TRANSPARENT);

        WindowInsetsControllerCompat controller =
                WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
        controller.setAppearanceLightStatusBars(false);
        controller.setAppearanceLightNavigationBars(false);

        super.onCreate(savedInstanceState);
    }

    @Override
    public WindowInsetsCompat onApplyWindowInsets(View view, WindowInsetsCompat insets) {
        Insets bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() | WindowInsetsCompat.Type.displayCutout());
        Insets ime = insets.getInsets(WindowInsetsCompat.Type.ime());

        nativeInsets(bars.top, Math.max(bars.bottom, ime.bottom), bars.left, bars.right);

        return super.onApplyWindowInsets(view, insets);
    }
}
