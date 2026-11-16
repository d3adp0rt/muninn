package io.github.d3adp0rt.muninn;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.graphics.Color;
import android.graphics.Typeface;
import android.os.Build;
import android.os.Bundle;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.text.method.ScrollingMovementMethod;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.WindowManager;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.window.OnBackInvokedDispatcher;

import androidx.core.graphics.Insets;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;

import com.google.androidgamesdk.GameActivity;

import java.security.KeyStore;

import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

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

    private native boolean nativeBack();

    private native void nativeInputText(int target, String text);

    private native void nativeInputSelection(int target, int start, int end);

    private native void nativeInputFocus(int target, boolean focused);

    private native void nativeInputAction(int target, int action);

    private EditText inputView;
    private int inputTarget = -1;
    private boolean inputSuppress = false;
    private InputMethodManager imeManager;

    private int dp(float value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }

    private int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    private void ensureInputView() {
        if (inputView != null) {
            return;
        }

        imeManager = (InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
        MuninnEditText view = new MuninnEditText(this);
        inputView = view;
        view.setBackgroundColor(Color.TRANSPARENT);
        view.setIncludeFontPadding(false);
        view.setPadding(dp(2), dp(8), dp(2), dp(8));
        view.setGravity(Gravity.TOP | Gravity.START);
        view.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f);
        view.setSingleLine(false);
        view.setMaxLines(5);
        view.setImeOptions(EditorInfo.IME_ACTION_NONE | EditorInfo.IME_FLAG_NO_FULLSCREEN);
        view.setInputType(InputType.TYPE_CLASS_TEXT
                | InputType.TYPE_TEXT_FLAG_MULTI_LINE
                | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
        try {
            view.setTypeface(Typeface.createFromAsset(getAssets(), "fonts/InterVariable.ttf"));
        } catch (Exception ignored) {
        }
        view.setVisibility(View.GONE);

        view.addTextChangedListener(new TextWatcher() {
            @Override
            public void beforeTextChanged(CharSequence s, int start, int count, int after) {
            }

            @Override
            public void onTextChanged(CharSequence s, int start, int before, int count) {
            }

            @Override
            public void afterTextChanged(Editable s) {
                if (!inputSuppress && inputTarget >= 0 && inputView != null
                        && inputView.isFocused()) {
                    nativeInputText(inputTarget, s.toString());
                }
            }
        });
        view.onSelectionChanged = () -> {
            if (!inputSuppress && inputTarget >= 0) {
                nativeInputSelection(
                        inputTarget, view.getSelectionStart(), view.getSelectionEnd());
            }
        };
        view.setOnFocusChangeListener((v, hasFocus) -> {
            if (inputTarget >= 0) {
                nativeInputFocus(inputTarget, hasFocus);
            }
        });
        view.setOnEditorActionListener((v, actionId, event) -> {
            if (inputTarget >= 0) {
                nativeInputAction(inputTarget, actionId);
                return true;
            }
            return false;
        });

        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(1, 1);
        params.gravity = Gravity.TOP | Gravity.START;
        addContentView(view, params);
        view.bringToFront();
    }

    /** Show/move the native text field at an absolute pixel rect. */
    public void muninnInputShow(int target, String geometry, String text, String hint) {
        runOnUiThread(() -> {
            ensureInputView();
            String[] p = geometry.split("\\|", -1);
            if (p.length < 12) {
                return;
            }

            int x = Integer.parseInt(p[0]);
            int y = Integer.parseInt(p[1]);
            int w = Math.max(1, Integer.parseInt(p[2]));
            int h = Math.max(1, Integer.parseInt(p[3]));
            int selStart = Integer.parseInt(p[4]);
            int selEnd = Integer.parseInt(p[5]);
            int textColor = Integer.parseInt(p[6]);
            int hintColor = Integer.parseInt(p[7]);
            int bgColor = Integer.parseInt(p[8]);
            float textSize = Float.parseFloat(p[9]);
            boolean multiline = "1".equals(p[10]);
            boolean password = "1".equals(p[11]);

            FrameLayout.LayoutParams params =
                    (FrameLayout.LayoutParams) inputView.getLayoutParams();
            params.leftMargin = x;
            params.topMargin = y;
            params.width = w;
            params.height = h;
            params.gravity = Gravity.TOP | Gravity.START;
            inputView.setLayoutParams(params);

            // A multi-pass egui frame can ask to show the same field twice;
            // keep the text and caret untouched in that case.
            if (target == inputTarget
                    && inputView.getVisibility() == View.VISIBLE
                    && inputView.isFocused()) {
                return;
            }

            // Configuration below can emit text callbacks; the field is not
            // live yet, so keep them from clobbering Rust state.
            inputSuppress = true;
            inputTarget = target;

            int type = InputType.TYPE_CLASS_TEXT;
            if (multiline) {
                type |= InputType.TYPE_TEXT_FLAG_MULTI_LINE;
                type |= InputType.TYPE_TEXT_FLAG_CAP_SENTENCES;
            }
            if (password) {
                type |= InputType.TYPE_TEXT_VARIATION_PASSWORD;
            }

            inputView.setTextSize(TypedValue.COMPLEX_UNIT_SP, textSize);
            inputView.setInputType(type);
            inputView.setMovementMethod(
                    multiline ? new ScrollingMovementMethod() : null);
            inputView.setVerticalScrollBarEnabled(multiline);
            inputView.setSingleLine(!multiline);
            inputView.setMaxLines(multiline ? 5 : 1);
            inputView.setTextColor(textColor);
            inputView.setHintTextColor(hintColor);
            inputView.setBackgroundColor(bgColor);
            inputView.setHint(hint);
            try {
                inputView.setTypeface(Typeface.createFromAsset(getAssets(), "fonts/InterVariable.ttf"));
            } catch (Exception ignored) {
            }

            inputView.setText(text);
            int len = inputView.getText().length();
            int start = selStart < 0 ? len : clamp(selStart, 0, len);
            int end = selEnd < 0 ? len : clamp(selEnd, 0, len);
            inputView.setSelection(start, end);

            inputView.setVisibility(View.VISIBLE);
            inputView.bringToFront();
            inputView.requestFocus();
            inputSuppress = false;
            inputView.postDelayed(() -> {
                if (imeManager != null) {
                    imeManager.showSoftInput(inputView, InputMethodManager.SHOW_IMPLICIT);
                }
            }, 80);
        });
    }

    /** Move the field while the keyboard/insets change, without touching text. */
    public void muninnInputSetRect(int x, int y, int w, int h) {
        runOnUiThread(() -> {
            if (inputView == null) {
                return;
            }
            FrameLayout.LayoutParams params =
                    (FrameLayout.LayoutParams) inputView.getLayoutParams();
            params.leftMargin = x;
            params.topMargin = y;
            params.width = Math.max(1, w);
            params.height = Math.max(1, h);
            inputView.setLayoutParams(params);
        });
    }

    /** Programmatic text update (send/clear) without echoing a change event. */
    public void muninnInputSetText(String text, int selStart, int selEnd) {
        runOnUiThread(() -> {
            if (inputView == null) {
                return;
            }
            inputSuppress = true;
            inputView.setText(text);
            int len = inputView.getText().length();
            inputView.setSelection(
                    selStart < 0 ? len : clamp(selStart, 0, len),
                    selEnd < 0 ? len : clamp(selEnd, 0, len));
            inputSuppress = false;
        });
    }

    public void muninnInputHide() {
        runOnUiThread(() -> {
            if (inputView == null) {
                return;
            }
            inputTarget = -1;
            if (imeManager != null) {
                imeManager.hideSoftInputFromWindow(inputView.getWindowToken(), 0);
            }
            inputView.clearFocus();
            inputView.setVisibility(View.GONE);
        });
    }

    /** EditText that reports selection changes (TextView hides the callback). */
    private static class MuninnEditText extends EditText {
        Runnable onSelectionChanged;

        MuninnEditText(Context context) {
            super(context);
        }

        @Override
        protected void onSelectionChanged(int selStart, int selEnd) {
            super.onSelectionChanged(selStart, selEnd);
            if (onSelectionChanged != null) {
                onSelectionChanged.run();
            }
        }
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING);

        getWindow().setStatusBarColor(Color.TRANSPARENT);
        getWindow().setNavigationBarColor(Color.TRANSPARENT);

        WindowInsetsControllerCompat controller =
                WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
        controller.setAppearanceLightStatusBars(false);
        controller.setAppearanceLightNavigationBars(false);

        super.onCreate(savedInstanceState);

        if (Build.VERSION.SDK_INT >= 33) {
            getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                    OnBackInvokedDispatcher.PRIORITY_DEFAULT,
                    () -> {
                        if (!nativeBack()) {
                            finish();
                        }
                    });
        }
    }

    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        if (!nativeBack()) {
            super.onBackPressed();
        }
    }

    @Override
    public WindowInsetsCompat onApplyWindowInsets(View view, WindowInsetsCompat insets) {
        Insets bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() | WindowInsetsCompat.Type.displayCutout());
        Insets ime = insets.getInsets(WindowInsetsCompat.Type.ime());

        nativeInsets(bars.top, Math.max(bars.bottom, ime.bottom), bars.left, bars.right);

        return super.onApplyWindowInsets(view, insets);
    }

    /** Switch system bar icon color for light/dark app themes. */
    public void muninnSetLightBars(boolean light) {
        runOnUiThread(() -> {
            WindowInsetsControllerCompat controller =
                    WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
            controller.setAppearanceLightStatusBars(light);
            controller.setAppearanceLightNavigationBars(light);
        });
    }

    /** IANA id of the device time zone, used to expand prompt variables. */
    public String muninnTimezone() {
        return java.util.TimeZone.getDefault().getID();
    }

    /** Read the primary clipboard item as text (null if empty). */
    public String muninnClipboardGet() {
        ClipboardManager manager = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
        if (manager == null || !manager.hasPrimaryClip()) {
            return null;
        }
        ClipData clip = manager.getPrimaryClip();
        if (clip == null || clip.getItemCount() == 0) {
            return null;
        }
        CharSequence text = clip.getItemAt(0).coerceToText(this);
        return text == null ? null : text.toString();
    }

    /** Replace the primary clipboard with plain text. */
    public void muninnClipboardSet(String text) {
        ClipboardManager manager = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
        if (manager != null) {
            manager.setPrimaryClip(ClipData.newPlainText("Muninn", text));
        }
    }

    private static final String KEY_ALIAS = "muninn_master_v1";

    private SecretKey masterKey() throws Exception {
        KeyStore store = KeyStore.getInstance("AndroidKeyStore");
        store.load(null);
        if (!store.containsAlias(KEY_ALIAS)) {
            KeyGenerator generator = KeyGenerator.getInstance(
                    KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
            generator.init(new KeyGenParameterSpec.Builder(
                    KEY_ALIAS,
                    KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setKeySize(256)
                    .build());
            generator.generateKey();
        }
        return ((KeyStore.SecretKeyEntry) store.getEntry(KEY_ALIAS, null)).getSecretKey();
    }

    /** Encrypt bytes with an AES-GCM key held by Android Keystore. */
    public byte[] muninnSeal(byte[] plaintext) {
        try {
            Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
            cipher.init(Cipher.ENCRYPT_MODE, masterKey());
            byte[] iv = cipher.getIV();
            byte[] ciphertext = cipher.doFinal(plaintext);
            byte[] out = new byte[iv.length + ciphertext.length];
            System.arraycopy(iv, 0, out, 0, iv.length);
            System.arraycopy(ciphertext, 0, out, iv.length, ciphertext.length);
            return out;
        } catch (Exception e) {
            e.printStackTrace();
            return null;
        }
    }

    /** Decrypt bytes produced by {@link #muninnSeal(byte[])}. */
    public byte[] muninnOpen(byte[] blob) {
        try {
            if (blob == null || blob.length < 13) {
                return null;
            }
            Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
            cipher.init(Cipher.DECRYPT_MODE, masterKey(), new GCMParameterSpec(128, blob, 0, 12));
            return cipher.doFinal(blob, 12, blob.length - 12);
        } catch (Exception e) {
            e.printStackTrace();
            return null;
        }
    }
}
