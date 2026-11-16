package io.github.d3adp0rt.muninn;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.graphics.Color;
import android.os.Bundle;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.view.View;

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
