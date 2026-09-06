import React, {Component, lazy, Suspense} from 'react';
import {AdditiveForceVisualizer} from 'shapjs';

import {DashComponentProps} from '../props';

type Props = {
    /**
     * The features of the model.
     */
    features: object,
    /**
     * The style of the component.
     */
    style?: object,
    /**
     * The title of the component.
     */
    title?: string,
    /**
     * The CSS class of the component.
     */
    className?: string,
    /**
     * The base value.
     */
    baseValue?: number,
    /**
     * plot color map.
     */
    plot_cmap?: "RdBu" | "GnPR" | "CyPU" | "PkYg" | "DrDb" | "LpLb" | "YlDp" | "OrId",
    /**
     * link function.
     */
    link?: "identity" | "logit",
    /**
     * feature names.
     */
    featureNames?: Array<string>,
    /**
     * The out names.
     */
    outNames?: Array<string>,
    /**
     * hide base value label.
     */
    hideBaseValueLabel?: boolean,
    /**
     * hide bars.
     */
    hideBars?: boolean,
    /**
     * label margin.
     */
    labelMargin?: number,
} & DashComponentProps;

/**
 * This is a description of the ForcePlot component.
 * The next thing to try to get the mouse over action to work
 * would likley be to migrate this:
 *  https://github.com/shap/shap/blob/master/javascript/visualizers/AdditiveForceVisualizer.jsx
 * into this repo and clean it up and update until it works.
 * Not important enough to do now.
 */
const ForcePlot = (props: Props) => {

    return (
        <div id={props.id} className={props.className} style={props.style}>
            <span
            style={{
                fontWeight:"bold",
                width:"100%",
                textAlign:"center",
                display:"block",
                fontSize:"15",
                }}
            >{props.title}</span>
            <div style={{margin: "10px", padding: "5px"}}>
                <AdditiveForceVisualizer {...props} />
            </div>
        </div>
    );
}

ForcePlot.defaultProps = {
    style: {},
    title: '',
    className: '',
    baseValue: 0,
    plot_cmap: 'RdBu',
    link: 'identity',
    featureNames: [],
    outNames: [],
    hideBaseValueLabel: false,
    hideBars: false,
    labelMargin: 20,
};

export default ForcePlot;
