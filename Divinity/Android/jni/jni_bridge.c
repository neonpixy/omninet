/**
 * Divinity JNI Bridge — thin C shim between Kotlin and the divi_* FFI.
 *
 * Each Kotlin `external` function gets a corresponding JNI-named C function
 * that converts JNI types to C types and calls through to the Rust FFI.
 *
 * Opaque pointers are stored as jlong (int64) in Kotlin.
 * Strings go through GetStringUTFChars / NewStringUTF.
 * Byte arrays go through Get/SetByteArrayRegion.
 */

#include <jni.h>
#include <stdlib.h>
#include <string.h>
#include "divinity_ffi.h"

/* --------------------------------------------------------------------------
 * Global JVM reference for callback thread attachment.
 * -------------------------------------------------------------------------- */

static JavaVM* g_jvm = NULL;

JNIEXPORT jint JNI_OnLoad(JavaVM* vm, void* reserved) {
    (void)reserved;
    g_jvm = vm;
    return JNI_VERSION_1_6;
}

/* --------------------------------------------------------------------------
 * Helpers
 * -------------------------------------------------------------------------- */

#define JNI_FN(cls, method) Java_com_omnidea_core_##cls##_##method

/** Get the last FFI error as a Java string, or null. */
JNIEXPORT jstring JNICALL
JNI_FN(OmnideaError, nativeLastError)(JNIEnv* env, jclass cls) {
    (void)cls;
    const char* err = divi_last_error();
    if (!err) return NULL;
    return (*env)->NewStringUTF(env, err);
}

/* --------------------------------------------------------------------------
 * Phone
 * -------------------------------------------------------------------------- */

JNIEXPORT jlong JNICALL
JNI_FN(Phone, nativeNew)(JNIEnv* env, jclass cls) {
    (void)env; (void)cls;
    return (jlong)(intptr_t)divi_phone_new();
}

JNIEXPORT void JNICALL
JNI_FN(Phone, nativeFree)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)env; (void)cls;
    if (ptr) divi_phone_free((Phone*)(intptr_t)ptr);
}

/**
 * Phone handler trampoline — called by Rust, calls back into Kotlin.
 *
 * The context is a JNI GlobalRef to a Kotlin PhoneHandler instance.
 * We attach the current thread to the JVM, call handler.handle(ByteArray),
 * and copy the response bytes into the out-parameters.
 */
static int32_t phone_handler_trampoline(
    const uint8_t* request_data, uintptr_t request_len,
    uint8_t** response_data, uintptr_t* response_len,
    void* context
) {
    if (!g_jvm || !context) return -1;

    JNIEnv* env = NULL;
    int attached = 0;
    jint status = (*g_jvm)->GetEnv(g_jvm, (void**)&env, JNI_VERSION_1_6);
    if (status == JNI_EDETACHED) {
        (*g_jvm)->AttachCurrentThread(g_jvm, (void**)&env, NULL);
        attached = 1;
    } else if (status != JNI_OK) {
        return -1;
    }

    jobject handler = (jobject)context;
    jclass handler_cls = (*env)->GetObjectClass(env, handler);
    jmethodID handle_mid = (*env)->GetMethodID(env, handler_cls, "handle", "([B)[B");

    // Create Java byte array from request.
    jbyteArray jreq = (*env)->NewByteArray(env, (jsize)request_len);
    if (request_data && request_len > 0) {
        (*env)->SetByteArrayRegion(env, jreq, 0, (jsize)request_len, (const jbyte*)request_data);
    }

    // Call Kotlin handler.
    jbyteArray jresp = (jbyteArray)(*env)->CallObjectMethod(env, handler, handle_mid, jreq);

    (*env)->DeleteLocalRef(env, jreq);
    (*env)->DeleteLocalRef(env, handler_cls);

    if (!jresp) {
        if (attached) (*g_jvm)->DetachCurrentThread(g_jvm);
        return -1;
    }

    // Copy response bytes.
    jsize resp_size = (*env)->GetArrayLength(env, jresp);
    *response_data = (uint8_t*)malloc((size_t)resp_size);
    *response_len = (uintptr_t)resp_size;
    (*env)->GetByteArrayRegion(env, jresp, 0, resp_size, (jbyte*)*response_data);
    (*env)->DeleteLocalRef(env, jresp);

    if (attached) (*g_jvm)->DetachCurrentThread(g_jvm);
    return 0;
}

JNIEXPORT void JNICALL
JNI_FN(Phone, nativeRegisterRaw)(JNIEnv* env, jclass cls, jlong ptr,
                                  jstring call_id, jobject handler) {
    (void)cls;
    const char* cid = (*env)->GetStringUTFChars(env, call_id, NULL);

    // Create a GlobalRef so the handler survives beyond this JNI call.
    jobject global_handler = (*env)->NewGlobalRef(env, handler);

    divi_phone_register_raw(
        (const Phone*)(intptr_t)ptr,
        cid,
        phone_handler_trampoline,
        (void*)global_handler
    );

    (*env)->ReleaseStringUTFChars(env, call_id, cid);
}

JNIEXPORT jbyteArray JNICALL
JNI_FN(Phone, nativeCallRaw)(JNIEnv* env, jclass cls, jlong ptr,
                              jstring call_id, jbyteArray data) {
    (void)cls;
    const char* cid = (*env)->GetStringUTFChars(env, call_id, NULL);
    jsize data_len = data ? (*env)->GetArrayLength(env, data) : 0;
    jbyte* data_bytes = data_len > 0 ? (*env)->GetByteArrayElements(env, data, NULL) : NULL;

    uint8_t* out_data = NULL;
    uintptr_t out_len = 0;

    int32_t result = divi_phone_call_raw(
        (const Phone*)(intptr_t)ptr,
        cid,
        (const uint8_t*)data_bytes,
        (uintptr_t)data_len,
        &out_data, &out_len
    );

    if (data_bytes) (*env)->ReleaseByteArrayElements(env, data, data_bytes, JNI_ABORT);
    (*env)->ReleaseStringUTFChars(env, call_id, cid);

    if (result != 0) return NULL;

    jbyteArray jresult = (*env)->NewByteArray(env, (jsize)out_len);
    if (out_data && out_len > 0) {
        (*env)->SetByteArrayRegion(env, jresult, 0, (jsize)out_len, (const jbyte*)out_data);
        divi_free_bytes(out_data, out_len);
    }
    return jresult;
}

JNIEXPORT jboolean JNICALL
JNI_FN(Phone, nativeHasHandler)(JNIEnv* env, jclass cls, jlong ptr, jstring call_id) {
    (void)cls;
    const char* cid = (*env)->GetStringUTFChars(env, call_id, NULL);
    jboolean result = divi_phone_has_handler((const Phone*)(intptr_t)ptr, cid) ? JNI_TRUE : JNI_FALSE;
    (*env)->ReleaseStringUTFChars(env, call_id, cid);
    return result;
}

JNIEXPORT jstring JNICALL
JNI_FN(Phone, nativeRegisteredCallIds)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)cls;
    char* json = divi_phone_registered_call_ids((const Phone*)(intptr_t)ptr);
    if (!json) return NULL;
    jstring result = (*env)->NewStringUTF(env, json);
    divi_free_string(json);
    return result;
}

JNIEXPORT void JNICALL
JNI_FN(Phone, nativeUnregister)(JNIEnv* env, jclass cls, jlong ptr, jstring call_id) {
    (void)cls;
    const char* cid = (*env)->GetStringUTFChars(env, call_id, NULL);
    divi_phone_unregister((const Phone*)(intptr_t)ptr, cid);
    (*env)->ReleaseStringUTFChars(env, call_id, cid);
}

/* --------------------------------------------------------------------------
 * Email
 * -------------------------------------------------------------------------- */

JNIEXPORT jlong JNICALL
JNI_FN(Email, nativeNew)(JNIEnv* env, jclass cls) {
    (void)env; (void)cls;
    return (jlong)(intptr_t)divi_email_new();
}

JNIEXPORT void JNICALL
JNI_FN(Email, nativeFree)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)env; (void)cls;
    if (ptr) divi_email_free((Email*)(intptr_t)ptr);
}

/** Email handler trampoline — fire-and-forget. */
static void email_handler_trampoline(const uint8_t* data, uintptr_t len, void* context) {
    if (!g_jvm || !context) return;

    JNIEnv* env = NULL;
    int attached = 0;
    jint status = (*g_jvm)->GetEnv(g_jvm, (void**)&env, JNI_VERSION_1_6);
    if (status == JNI_EDETACHED) {
        (*g_jvm)->AttachCurrentThread(g_jvm, (void**)&env, NULL);
        attached = 1;
    } else if (status != JNI_OK) {
        return;
    }

    jobject handler = (jobject)context;
    jclass handler_cls = (*env)->GetObjectClass(env, handler);
    jmethodID handle_mid = (*env)->GetMethodID(env, handler_cls, "handle", "([B)V");

    jbyteArray jdata = (*env)->NewByteArray(env, (jsize)len);
    if (data && len > 0) {
        (*env)->SetByteArrayRegion(env, jdata, 0, (jsize)len, (const jbyte*)data);
    }

    (*env)->CallVoidMethod(env, handler, handle_mid, jdata);

    (*env)->DeleteLocalRef(env, jdata);
    (*env)->DeleteLocalRef(env, handler_cls);
    if (attached) (*g_jvm)->DetachCurrentThread(g_jvm);
}

JNIEXPORT jbyteArray JNICALL
JNI_FN(Email, nativeSubscribeRaw)(JNIEnv* env, jclass cls, jlong ptr,
                                   jstring email_id, jobject handler) {
    (void)cls;
    const char* eid = (*env)->GetStringUTFChars(env, email_id, NULL);
    jobject global_handler = (*env)->NewGlobalRef(env, handler);

    uint8_t uuid_bytes[16] = {0};
    int32_t result = divi_email_subscribe_raw(
        (const Email*)(intptr_t)ptr,
        eid,
        email_handler_trampoline,
        (void*)global_handler,
        uuid_bytes
    );

    (*env)->ReleaseStringUTFChars(env, email_id, eid);

    if (result != 0) {
        (*env)->DeleteGlobalRef(env, global_handler);
        return NULL;
    }

    jbyteArray juuid = (*env)->NewByteArray(env, 16);
    (*env)->SetByteArrayRegion(env, juuid, 0, 16, (const jbyte*)uuid_bytes);
    return juuid;
}

JNIEXPORT void JNICALL
JNI_FN(Email, nativeSendRaw)(JNIEnv* env, jclass cls, jlong ptr,
                              jstring email_id, jbyteArray data) {
    (void)cls;
    const char* eid = (*env)->GetStringUTFChars(env, email_id, NULL);
    jsize data_len = data ? (*env)->GetArrayLength(env, data) : 0;
    jbyte* data_bytes = data_len > 0 ? (*env)->GetByteArrayElements(env, data, NULL) : NULL;

    divi_email_send_raw(
        (const Email*)(intptr_t)ptr,
        eid,
        (const uint8_t*)data_bytes,
        (uintptr_t)data_len
    );

    if (data_bytes) (*env)->ReleaseByteArrayElements(env, data, data_bytes, JNI_ABORT);
    (*env)->ReleaseStringUTFChars(env, email_id, eid);
}

JNIEXPORT void JNICALL
JNI_FN(Email, nativeUnsubscribe)(JNIEnv* env, jclass cls, jlong ptr, jbyteArray uuid) {
    (void)cls;
    jbyte* uuid_bytes = (*env)->GetByteArrayElements(env, uuid, NULL);
    divi_email_unsubscribe((const Email*)(intptr_t)ptr, (const uint8_t*)uuid_bytes);
    (*env)->ReleaseByteArrayElements(env, uuid, uuid_bytes, JNI_ABORT);
}

JNIEXPORT void JNICALL
JNI_FN(Email, nativeUnsubscribeAll)(JNIEnv* env, jclass cls, jlong ptr, jstring email_id) {
    (void)cls;
    const char* eid = (*env)->GetStringUTFChars(env, email_id, NULL);
    divi_email_unsubscribe_all((const Email*)(intptr_t)ptr, eid);
    (*env)->ReleaseStringUTFChars(env, email_id, eid);
}

JNIEXPORT jboolean JNICALL
JNI_FN(Email, nativeHasSubscribers)(JNIEnv* env, jclass cls, jlong ptr, jstring email_id) {
    (void)cls;
    const char* eid = (*env)->GetStringUTFChars(env, email_id, NULL);
    jboolean result = divi_email_has_subscribers((const Email*)(intptr_t)ptr, eid) ? JNI_TRUE : JNI_FALSE;
    (*env)->ReleaseStringUTFChars(env, email_id, eid);
    return result;
}

JNIEXPORT jstring JNICALL
JNI_FN(Email, nativeActiveEmailIds)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)cls;
    char* json = divi_email_active_email_ids((const Email*)(intptr_t)ptr);
    if (!json) return NULL;
    jstring result = (*env)->NewStringUTF(env, json);
    divi_free_string(json);
    return result;
}

/* --------------------------------------------------------------------------
 * Contacts
 * -------------------------------------------------------------------------- */

JNIEXPORT jlong JNICALL
JNI_FN(Contacts, nativeNew)(JNIEnv* env, jclass cls) {
    (void)env; (void)cls;
    return (jlong)(intptr_t)divi_contacts_new();
}

JNIEXPORT void JNICALL
JNI_FN(Contacts, nativeFree)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)env; (void)cls;
    if (ptr) divi_contacts_free((Contacts*)(intptr_t)ptr);
}

JNIEXPORT jint JNICALL
JNI_FN(Contacts, nativeRegister)(JNIEnv* env, jclass cls, jlong ptr, jstring json) {
    (void)cls;
    const char* cjson = (*env)->GetStringUTFChars(env, json, NULL);
    jint result = divi_contacts_register((const Contacts*)(intptr_t)ptr, cjson);
    (*env)->ReleaseStringUTFChars(env, json, cjson);
    return result;
}

/** Shutdown callback trampoline. */
static void shutdown_trampoline(void* context) {
    if (!g_jvm || !context) return;

    JNIEnv* env = NULL;
    int attached = 0;
    jint status = (*g_jvm)->GetEnv(g_jvm, (void**)&env, JNI_VERSION_1_6);
    if (status == JNI_EDETACHED) {
        (*g_jvm)->AttachCurrentThread(g_jvm, (void**)&env, NULL);
        attached = 1;
    } else if (status != JNI_OK) {
        return;
    }

    jobject callback = (jobject)context;
    jclass cb_cls = (*env)->GetObjectClass(env, callback);
    jmethodID on_shutdown = (*env)->GetMethodID(env, cb_cls, "onShutdown", "()V");

    (*env)->CallVoidMethod(env, callback, on_shutdown);

    (*env)->DeleteLocalRef(env, cb_cls);
    (*env)->DeleteGlobalRef(env, callback);  // One-shot: release after firing.
    if (attached) (*g_jvm)->DetachCurrentThread(g_jvm);
}

JNIEXPORT jint JNICALL
JNI_FN(Contacts, nativeRegisterWithShutdown)(JNIEnv* env, jclass cls, jlong ptr,
                                              jstring json, jobject callback) {
    (void)cls;
    const char* cjson = (*env)->GetStringUTFChars(env, json, NULL);
    jobject global_cb = (*env)->NewGlobalRef(env, callback);

    jint result = divi_contacts_register_with_shutdown(
        (const Contacts*)(intptr_t)ptr,
        cjson,
        shutdown_trampoline,
        (void*)global_cb
    );

    (*env)->ReleaseStringUTFChars(env, json, cjson);
    if (result != 0) (*env)->DeleteGlobalRef(env, global_cb);
    return result;
}

JNIEXPORT jint JNICALL
JNI_FN(Contacts, nativeUnregister)(JNIEnv* env, jclass cls, jlong ptr, jstring module_id) {
    (void)cls;
    const char* mid = (*env)->GetStringUTFChars(env, module_id, NULL);
    jint result = divi_contacts_unregister((const Contacts*)(intptr_t)ptr, mid);
    (*env)->ReleaseStringUTFChars(env, module_id, mid);
    return result;
}

JNIEXPORT void JNICALL
JNI_FN(Contacts, nativeShutdownAll)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)env; (void)cls;
    divi_contacts_shutdown_all((const Contacts*)(intptr_t)ptr);
}

JNIEXPORT jstring JNICALL
JNI_FN(Contacts, nativeLookup)(JNIEnv* env, jclass cls, jlong ptr, jstring module_id) {
    (void)cls;
    const char* mid = (*env)->GetStringUTFChars(env, module_id, NULL);
    char* json = divi_contacts_lookup((const Contacts*)(intptr_t)ptr, mid);
    (*env)->ReleaseStringUTFChars(env, module_id, mid);
    if (!json) return NULL;
    jstring result = (*env)->NewStringUTF(env, json);
    divi_free_string(json);
    return result;
}

JNIEXPORT jstring JNICALL
JNI_FN(Contacts, nativeAllModules)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)cls;
    char* json = divi_contacts_all_modules((const Contacts*)(intptr_t)ptr);
    if (!json) return NULL;
    jstring result = (*env)->NewStringUTF(env, json);
    divi_free_string(json);
    return result;
}

JNIEXPORT jstring JNICALL
JNI_FN(Contacts, nativeRegisteredModuleIds)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)cls;
    char* json = divi_contacts_registered_module_ids((const Contacts*)(intptr_t)ptr);
    if (!json) return NULL;
    jstring result = (*env)->NewStringUTF(env, json);
    divi_free_string(json);
    return result;
}

JNIEXPORT jstring JNICALL
JNI_FN(Contacts, nativeDependentsOf)(JNIEnv* env, jclass cls, jlong ptr, jstring module_id) {
    (void)cls;
    const char* mid = (*env)->GetStringUTFChars(env, module_id, NULL);
    char* json = divi_contacts_dependents_of((const Contacts*)(intptr_t)ptr, mid);
    (*env)->ReleaseStringUTFChars(env, module_id, mid);
    if (!json) return NULL;
    jstring result = (*env)->NewStringUTF(env, json);
    divi_free_string(json);
    return result;
}

/* --------------------------------------------------------------------------
 * Pager
 * -------------------------------------------------------------------------- */

JNIEXPORT jlong JNICALL
JNI_FN(Pager, nativeNew)(JNIEnv* env, jclass cls) {
    (void)env; (void)cls;
    return (jlong)(intptr_t)divi_pager_new();
}

JNIEXPORT void JNICALL
JNI_FN(Pager, nativeFree)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)env; (void)cls;
    if (ptr) divi_pager_free((Pager*)(intptr_t)ptr);
}

JNIEXPORT jstring JNICALL
JNI_FN(Pager, nativeNotify)(JNIEnv* env, jclass cls, jlong ptr, jstring json) {
    (void)cls;
    const char* cjson = (*env)->GetStringUTFChars(env, json, NULL);
    char* uuid = divi_pager_notify((const Pager*)(intptr_t)ptr, cjson);
    (*env)->ReleaseStringUTFChars(env, json, cjson);
    if (!uuid) return NULL;
    jstring result = (*env)->NewStringUTF(env, uuid);
    divi_free_string(uuid);
    return result;
}

JNIEXPORT jstring JNICALL
JNI_FN(Pager, nativeGetPending)(JNIEnv* env, jclass cls, jlong ptr, jstring priority_json) {
    (void)cls;
    const char* cpri = priority_json ? (*env)->GetStringUTFChars(env, priority_json, NULL) : NULL;
    char* json = divi_pager_get_pending((const Pager*)(intptr_t)ptr, cpri);
    if (cpri) (*env)->ReleaseStringUTFChars(env, priority_json, cpri);
    if (!json) return NULL;
    jstring result = (*env)->NewStringUTF(env, json);
    divi_free_string(json);
    return result;
}

JNIEXPORT jstring JNICALL
JNI_FN(Pager, nativeGetUnread)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)cls;
    char* json = divi_pager_get_unread((const Pager*)(intptr_t)ptr);
    if (!json) return NULL;
    jstring result = (*env)->NewStringUTF(env, json);
    divi_free_string(json);
    return result;
}

JNIEXPORT jboolean JNICALL
JNI_FN(Pager, nativeMarkRead)(JNIEnv* env, jclass cls, jlong ptr, jstring uuid_str) {
    (void)cls;
    const char* cuuid = (*env)->GetStringUTFChars(env, uuid_str, NULL);
    jboolean result = divi_pager_mark_read((const Pager*)(intptr_t)ptr, cuuid) ? JNI_TRUE : JNI_FALSE;
    (*env)->ReleaseStringUTFChars(env, uuid_str, cuuid);
    return result;
}

JNIEXPORT jboolean JNICALL
JNI_FN(Pager, nativeDismiss)(JNIEnv* env, jclass cls, jlong ptr, jstring uuid_str) {
    (void)cls;
    const char* cuuid = (*env)->GetStringUTFChars(env, uuid_str, NULL);
    jboolean result = divi_pager_dismiss((const Pager*)(intptr_t)ptr, cuuid) ? JNI_TRUE : JNI_FALSE;
    (*env)->ReleaseStringUTFChars(env, uuid_str, cuuid);
    return result;
}

JNIEXPORT jlong JNICALL
JNI_FN(Pager, nativeBadgeCount)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)env; (void)cls;
    return (jlong)divi_pager_badge_count((const Pager*)(intptr_t)ptr);
}

JNIEXPORT void JNICALL
JNI_FN(Pager, nativePruneExpired)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)env; (void)cls;
    divi_pager_prune_expired((const Pager*)(intptr_t)ptr);
}

JNIEXPORT jstring JNICALL
JNI_FN(Pager, nativeExportState)(JNIEnv* env, jclass cls, jlong ptr) {
    (void)cls;
    char* json = divi_pager_export_state((const Pager*)(intptr_t)ptr);
    if (!json) return NULL;
    jstring result = (*env)->NewStringUTF(env, json);
    divi_free_string(json);
    return result;
}

JNIEXPORT void JNICALL
JNI_FN(Pager, nativeRestoreState)(JNIEnv* env, jclass cls, jlong ptr, jstring json) {
    (void)cls;
    const char* cjson = (*env)->GetStringUTFChars(env, json, NULL);
    divi_pager_restore_state((const Pager*)(intptr_t)ptr, cjson);
    (*env)->ReleaseStringUTFChars(env, json, cjson);
}
